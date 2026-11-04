//! Focus peaking on the device: the passes of `peaking.wgsl` over the
//! target after its draw, held to [`crate::peaking`]. Nothing is made
//! until peaking is first on, and nothing runs while it is off.

use super::*;
use crate::peaking::{self as reference, Rect};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    size: [u32; 2],
    radius: u32,
    rects: u32,
    low: f32,
    steps: f32,
    bins: u32,
    below: u32,
    quantile: f32,
    shadow: f32,
    pad: [f32; 2],
    color: [f32; 4],
    rect: [[u32; 4]; 4],
}

/// The most rectangles a target is measured in: the compare view's
/// four tiles.
pub const MOST_RECTS: usize = 4;

/// The rectangle of a target the picture of `v` covers when drawn
/// into `tile` (x, y, width, height), a pixel in from each edge so the
/// canvas's step is not taken for detail; `None` when it covers none.
/// The shader's test of a pixel, `fs_main`'s first line, solved for
/// the pixel.
pub fn shown_rect(v: &View, tile: (u32, u32, u32, u32)) -> Option<Rect> {
    let (ox, oy, tw, th) = tile;
    let edge = |o: u32, t: u32, center: f32, at: f32| {
        // A pixel's center c is in when 0 <= (c - o - t/2)/zoom + center < frame.
        let c = o as f32 + t as f32 / 2.0 + (at - center) * v.zoom;
        (c - 0.5).ceil().clamp(o as f32, (o + t) as f32) as u32
    };
    let x0 = edge(ox, tw, v.center.0, 0.0) + 1;
    let x1 = edge(ox, tw, v.center.0, v.frame_size.0).saturating_sub(1);
    let y0 = edge(oy, th, v.center.1, 0.0) + 1;
    let y1 = edge(oy, th, v.center.1, v.frame_size.1).saturating_sub(1);
    (x1 > x0 + 2 && y1 > y0 + 2).then_some([x0, y0, x1, y1])
}

pub(super) struct Peaking {
    compute_layout: gpu::BindGroupLayout,
    paint_layout: gpu::BindGroupLayout,
    passes: [gpu::ComputePipeline; 4],
    pick: gpu::ComputePipeline,
    paint: gpu::RenderPipeline,
    /// The counts and the thresholds, a set a rectangle, and the
    /// uniform, written each frame.
    counts: gpu::Buffer,
    threshold: gpu::Buffer,
    uniform: gpu::Buffer,
    /// What is made for one target: kept while the target is.
    made: Option<Made>,
}

/// The two bind groups for one target, which hold the map's two
/// planes at its size.
struct Made {
    target: gpu::Texture,
    compute: gpu::BindGroup,
    paint: gpu::BindGroup,
}

fn storage_entry(
    binding: u32,
    visibility: gpu::ShaderStages,
    read_only: bool,
) -> gpu::BindGroupLayoutEntry {
    gpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: gpu::BindingType::Buffer {
            ty: gpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(visibility: gpu::ShaderStages) -> gpu::BindGroupLayoutEntry {
    gpu::BindGroupLayoutEntry {
        binding: 0,
        visibility,
        ty: gpu::BindingType::Buffer {
            ty: gpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl Peaking {
    pub(super) fn new(device: &gpu::Device) -> Self {
        let shader = device.create_shader_module(gpu::ShaderModuleDescriptor {
            label: Some("peaking"),
            source: gpu::ShaderSource::Wgsl(include_str!("../peaking.wgsl").into()),
        });
        let compute = gpu::ShaderStages::COMPUTE;
        let compute_layout = device.create_bind_group_layout(&gpu::BindGroupLayoutDescriptor {
            label: Some("peaking"),
            entries: &[
                uniform_entry(compute),
                gpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: compute,
                    ty: gpu::BindingType::Texture {
                        sample_type: gpu::TextureSampleType::Float { filterable: true },
                        view_dimension: gpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                storage_entry(2, compute, false),
                storage_entry(3, compute, false),
                storage_entry(4, compute, false),
                storage_entry(5, compute, false),
            ],
        });
        let fragment = gpu::ShaderStages::FRAGMENT;
        let paint_layout = device.create_bind_group_layout(&gpu::BindGroupLayoutDescriptor {
            label: Some("peaking paint"),
            entries: &[
                uniform_entry(fragment),
                storage_entry(2, fragment, true),
                storage_entry(5, fragment, true),
            ],
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&gpu::PipelineLayoutDescriptor {
                label: Some("peaking"),
                bind_group_layouts: &[Some(&compute_layout)],
                ..Default::default()
            });
        let pass = |entry: &str| {
            device.create_compute_pipeline(&gpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&compute_pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let passes = [
            pass("energy"),
            pass("blur_rows"),
            pass("blur_columns"),
            pass("count"),
        ];
        let pick = pass("pick");
        let paint_pipeline_layout = device.create_pipeline_layout(&gpu::PipelineLayoutDescriptor {
            label: Some("peaking paint"),
            bind_group_layouts: &[Some(&paint_layout)],
            ..Default::default()
        });
        let paint = device.create_render_pipeline(&gpu::RenderPipelineDescriptor {
            label: Some("peaking paint"),
            layout: Some(&paint_pipeline_layout),
            vertex: gpu::VertexState {
                module: &shader,
                entry_point: Some("vs_paint"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(gpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_paint"),
                compilation_options: Default::default(),
                targets: &[Some(TARGET_FORMAT.into())],
            }),
            primitive: gpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: gpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let small = |label: &str, size: u64| {
            device.create_buffer(&gpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: gpu::BufferUsages::STORAGE | gpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let uniform = device.create_buffer(&gpu::BufferDescriptor {
            label: Some("peaking params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: gpu::BufferUsages::UNIFORM | gpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            compute_layout,
            paint_layout,
            passes,
            pick,
            paint,
            counts: small("peaking counts", (reference::BINS * MOST_RECTS * 4) as u64),
            threshold: small("peaking thresholds", (MOST_RECTS * 4) as u64),
            uniform,
            made: None,
        }
    }

    /// Paint the peaks of what `target` holds, inside `rects`, in
    /// `color` (encoded, as the target holds it).
    pub(super) fn encode(
        &mut self,
        device: &gpu::Device,
        queue: &gpu::Queue,
        encoder: &mut gpu::CommandEncoder,
        target: &gpu::Texture,
        rects: &[Rect],
        color: [f32; 3],
    ) {
        if rects.is_empty() {
            return;
        }
        let (w, h) = (target.width(), target.height());
        if self.made.as_ref().is_none_or(|m| m.target != *target) {
            self.made = Some(self.make(device, target));
        }
        let made = self.made.as_ref().expect("made above");
        let rects = &rects[..rects.len().min(MOST_RECTS)];
        let mut rect = [[0u32; 4]; 4];
        for (r, from) in rect.iter_mut().zip(rects) {
            *r = *from;
        }
        let params = Params {
            size: [w, h],
            radius: reference::RADIUS as u32,
            rects: rects.len() as u32,
            low: reference::LOW,
            steps: reference::STEPS,
            bins: reference::BINS as u32,
            below: (reference::BELOW * reference::STEPS) as u32,
            quantile: reference::QUANTILE,
            shadow: reference::SHADOW.exp2(),
            pad: [0.0; 2],
            color: [color[0], color[1], color[2], 1.0],
            rect,
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&params));
        encoder.clear_buffer(&self.counts, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&gpu::ComputePassDescriptor {
                label: Some("peaking"),
                timestamp_writes: None,
            });
            pass.set_bind_group(0, &made.compute, &[]);
            for pipeline in &self.passes {
                pass.set_pipeline(pipeline);
                pass.dispatch_workgroups(w.div_ceil(16), h.div_ceil(16), 1);
            }
            pass.set_pipeline(&self.pick);
            pass.dispatch_workgroups(rects.len() as u32, 1, 1);
        }
        let shown = target.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&gpu::RenderPassDescriptor {
            label: Some("peaking paint"),
            color_attachments: &[Some(gpu::RenderPassColorAttachment {
                view: &shown,
                resolve_target: None,
                ops: gpu::Operations {
                    load: gpu::LoadOp::Load,
                    store: gpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.paint);
        pass.set_bind_group(0, &made.paint, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The map's planes for `target` and the bind groups over them.
    fn make(&self, device: &gpu::Device, target: &gpu::Texture) -> Made {
        let (w, h) = (target.width(), target.height());
        let plane = |label: &str| {
            device.create_buffer(&gpu::BufferDescriptor {
                label: Some(label),
                size: w as u64 * h as u64 * 4,
                usage: gpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            })
        };
        let (a, b) = (plane("peaking map"), plane("peaking rows"));
        let shown = target.create_view(&Default::default());
        let compute = device.create_bind_group(&gpu::BindGroupDescriptor {
            label: Some("peaking"),
            layout: &self.compute_layout,
            entries: &[
                gpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                gpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu::BindingResource::TextureView(&shown),
                },
                gpu::BindGroupEntry {
                    binding: 2,
                    resource: a.as_entire_binding(),
                },
                gpu::BindGroupEntry {
                    binding: 3,
                    resource: b.as_entire_binding(),
                },
                gpu::BindGroupEntry {
                    binding: 4,
                    resource: self.counts.as_entire_binding(),
                },
                gpu::BindGroupEntry {
                    binding: 5,
                    resource: self.threshold.as_entire_binding(),
                },
            ],
        });
        let paint = device.create_bind_group(&gpu::BindGroupDescriptor {
            label: Some("peaking paint"),
            layout: &self.paint_layout,
            entries: &[
                gpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                gpu::BindGroupEntry {
                    binding: 2,
                    resource: a.as_entire_binding(),
                },
                gpu::BindGroupEntry {
                    binding: 5,
                    resource: self.threshold.as_entire_binding(),
                },
            ],
        });
        Made {
            target: target.clone(),
            compute,
            paint,
        }
    }
}

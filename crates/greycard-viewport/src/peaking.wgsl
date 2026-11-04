// Focus peaking over the viewport's target, after its draw: the map
// of `greycard_core::sharpness` on the pixels as displayed, the
// frame's threshold from a count of the map in bins, and the paint.
// `peaking.rs` is the reference, and every constant comes from it
// through `P`.
//
// The passes, in order: `energy` (the squared Laplacian of the log
// luminance, its taps clamped to the pixel's rectangle), `blur_rows`
// and `blur_columns` (the mean over a 2r+1 window cut at the
// rectangle's edge), `count` (the bins, a set a rectangle), `pick`
// (each rectangle's threshold, a thread each), then the render pass
// `vs_paint`/`fs_paint`.

struct P {
    size: vec2<u32>,
    radius: u32,
    rects: u32,
    // The bins: the first's log2, steps an octave, how many; then the
    // quantile and how many bins under it still paint.
    low: f32,
    steps: f32,
    bins: u32,
    below: u32,
    quantile: f32,
    // The light added under the log, linear.
    shadow: f32,
    pad0: f32,
    pad1: f32,
    color: vec4<f32>,
    // Each rectangle: x0, y0, x1, y1, the far edges outside.
    rect: array<vec4<u32>, 4>,
}

@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(1) var shown: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> a: array<f32>;
@group(0) @binding(3) var<storage, read_write> b: array<f32>;
@group(0) @binding(4) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> threshold: array<u32>;

// The rectangle holding a pixel, or 4 for none.
fn rect_of(x: u32, y: u32) -> u32 {
    for (var k = 0u; k < min(p.rects, 4u); k = k + 1u) {
        let r = p.rect[k];
        if (x >= r.x && x < r.z && y >= r.y && y < r.w) {
            return k;
        }
    }
    return 4u;
}

fn linear(c: f32) -> f32 {
    if (c <= 0.04045) {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn log_lum(x: u32, y: u32) -> f32 {
    let c = textureLoad(shown, vec2<i32>(i32(x), i32(y)), 0).rgb;
    // The target holds eight bits; read them as the CPU does.
    let v = round(c * 255.0) / 255.0;
    let lum = 0.2126 * linear(v.r) + 0.7152 * linear(v.g) + 0.0722 * linear(v.b);
    return log2(lum + p.shadow);
}

@compute @workgroup_size(16, 16)
fn energy(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.size.x || id.y >= p.size.y) {
        return;
    }
    let i = id.y * p.size.x + id.x;
    let k = rect_of(id.x, id.y);
    if (k == 4u) {
        a[i] = 0.0;
        return;
    }
    let r = p.rect[k];
    let mid = log_lum(id.x, id.y);
    let left = log_lum(max(id.x, r.x + 1u) - 1u, id.y);
    let right = log_lum(min(id.x + 1u, r.z - 1u), id.y);
    let up = log_lum(id.x, max(id.y, r.y + 1u) - 1u);
    let down = log_lum(id.x, min(id.y + 1u, r.w - 1u));
    let lap = 4.0 * mid - left - right - up - down;
    a[i] = lap * lap;
}

@compute @workgroup_size(16, 16)
fn blur_rows(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.size.x || id.y >= p.size.y) {
        return;
    }
    let i = id.y * p.size.x + id.x;
    let k = rect_of(id.x, id.y);
    if (k == 4u) {
        b[i] = 0.0;
        return;
    }
    let r = p.rect[k];
    let lo = max(id.x, r.x + p.radius) - p.radius;
    let hi = min(id.x + p.radius, r.z - 1u);
    var sum = 0.0;
    for (var x = lo; x <= hi; x = x + 1u) {
        sum = sum + a[id.y * p.size.x + x];
    }
    b[i] = sum / f32(hi - lo + 1u);
}

@compute @workgroup_size(16, 16)
fn blur_columns(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.size.x || id.y >= p.size.y) {
        return;
    }
    let i = id.y * p.size.x + id.x;
    let k = rect_of(id.x, id.y);
    if (k == 4u) {
        a[i] = 0.0;
        return;
    }
    let r = p.rect[k];
    let lo = max(id.y, r.y + p.radius) - p.radius;
    let hi = min(id.y + p.radius, r.w - 1u);
    var sum = 0.0;
    for (var y = lo; y <= hi; y = y + 1u) {
        sum = sum + b[y * p.size.x + id.x];
    }
    a[i] = sum / f32(hi - lo + 1u);
}

fn bin(e: f32) -> u32 {
    if (!(e > 0.0)) {
        return 0u;
    }
    let v = floor((log2(e) - p.low) * p.steps);
    return u32(clamp(v, 0.0, f32(p.bins - 1u)));
}

@compute @workgroup_size(16, 16)
fn count(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.size.x || id.y >= p.size.y) {
        return;
    }
    let r = rect_of(id.x, id.y);
    if (r == 4u) {
        return;
    }
    atomicAdd(&counts[r * p.bins + bin(a[id.y * p.size.x + id.x])], 1u);
}

// One workgroup a rectangle.
@compute @workgroup_size(1)
fn pick(@builtin(workgroup_id) g: vec3<u32>) {
    let base = g.x * p.bins;
    var total = 0u;
    for (var k = 0u; k < p.bins; k = k + 1u) {
        total = total + atomicLoad(&counts[base + k]);
    }
    // As `peaking::threshold`: less a thousandth, so the rounding of
    // the share cannot ask for one count more.
    let want = max(u32(ceil(f32(total) * p.quantile - 1e-3)), 1u);
    var seen = 0u;
    var at = p.bins - 1u;
    for (var k = 0u; k < p.bins; k = k + 1u) {
        seen = seen + atomicLoad(&counts[base + k]);
        if (seen >= want) {
            at = k;
            break;
        }
    }
    threshold[g.x] = max(max(at, p.below) - p.below, 1u);
}

// The paint: one triangle over the target, each pixel the color where
// it peaks and left as it is elsewhere. Its pipeline's layout has the
// uniform, the map and the thresholds alone, since the target is what
// it draws into and cannot be read in the same pass, and the two
// buffers read only, as every backend takes them in a fragment
// shader: the same bindings as `a` and `threshold`, which no entry
// point uses beside these.
@group(0) @binding(2) var<storage, read> peaks: array<f32>;
@group(0) @binding(5) var<storage, read> from_bin: array<u32>;

@vertex
fn vs_paint(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_paint(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let x = u32(pos.x);
    let y = u32(pos.y);
    let r = rect_of(x, y);
    if (r == 4u || bin(peaks[y * p.size.x + x]) < from_bin[r]) {
        discard;
    }
    return vec4<f32>(p.color.rgb, 1.0);
}

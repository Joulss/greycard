// What the demosaic's kernels share (`rcd.wgsl`, `vng4.wgsl`,
// `dual.wgsl` follow this file in one module): the band's size, the
// mosaic, the working planes, and the reference's bilinear, which
// seeds RCD's buffers and fills both algorithms' borders.
//
// A band is a run of whole rows of the frame starting on an even row,
// so its CFA phases are the frame's. Every plane is a band's rows,
// one float a site, row after row; `rgb` is interleaved.

struct Params {
    size: vec2<u32>,
    // 0: RCD alone, 1: VNG4 alone, 2: the dual's blend.
    mode: u32,
    // The largest sample, and at least 1: where both clamp.
    ceiling: f32,
    // The RGB index of each phase, `(y & 1) * 2 + (x & 1)`.
    colors: vec4<u32>,
    // VNG4's four colors of each phase: red 0, green on red rows 1,
    // blue 2, green on blue rows 3.
    fc4: vec4<u32>,
    // Each phase's run of gradient terms in `terms`.
    term_start: vec4<u32>,
    term_count: vec4<u32>,
}

struct Term {
    d1: vec2<i32>,
    d2: vec2<i32>,
    weight: f32,
    grads: u32,
    pad0: u32,
    pad1: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> mosaic: array<f32>;
@group(0) @binding(2) var<storage, read_write> plane_a: array<f32>;
@group(0) @binding(3) var<storage, read_write> plane_b: array<f32>;
@group(0) @binding(4) var<storage, read_write> plane_c: array<f32>;
@group(0) @binding(5) var<storage, read_write> plane_d: array<f32>;
@group(0) @binding(6) var<storage, read_write> rgb: array<f32>;
// The mask's blur kernel, seven taps from the center out, then the
// threshold at each unit of L* from 0 to 100.
@group(0) @binding(7) var<storage, read> consts: array<f32>;
@group(0) @binding(8) var<storage, read> terms: array<Term>;

const EPS: f32 = 1e-5;
const EPS_SQ: f32 = 1e-10;

fn band_w() -> i32 {
    return i32(params.size.x);
}

fn band_h() -> i32 {
    return i32(params.size.y);
}

fn at(x: i32, y: i32) -> u32 {
    return u32(y * band_w() + x);
}

fn inside(id: vec3<u32>) -> bool {
    return id.x < params.size.x && id.y < params.size.y;
}

fn s(x: i32, y: i32) -> f32 {
    return mosaic[at(x, y)];
}

fn phase(x: i32, y: i32) -> u32 {
    return u32(((y & 1) << 1) | (x & 1));
}

fn color(x: i32, y: i32) -> u32 {
    return params.colors[phase(x, y)];
}

fn is_green(x: i32, y: i32) -> bool {
    return color(x, y) == 1u;
}

// Whether a site is `b` or more in from every edge of the band.
fn interior(x: i32, y: i32, b: i32) -> bool {
    return x >= b && x < band_w() - b && y >= b && y < band_h() - b;
}

// The reference's `bilinear_pixel`, color `c`: the site's own sample,
// or the mean of that color among its eight neighbors, row by row.
fn bilinear(x: i32, y: i32, c: u32) -> f32 {
    if c == color(x, y) {
        return s(x, y);
    }
    var sum = 0.0;
    var count = 0u;
    for (var ny = max(y - 1, 0); ny <= min(y + 1, band_h() - 1); ny++) {
        for (var nx = max(x - 1, 0); nx <= min(x + 1, band_w() - 1); nx++) {
            if (ny == y && nx == x) || color(nx, ny) != c {
                continue;
            }
            sum += s(nx, ny);
            count++;
        }
    }
    return sum / f32(count);
}

fn put_rgb(i: u32, v: vec3<f32>) {
    rgb[3u * i] = v.x;
    rgb[3u * i + 1u] = v.y;
    rgb[3u * i + 2u] = v.z;
}

fn get_rgb(i: u32) -> vec3<f32> {
    return vec3<f32>(rgb[3u * i], rgb[3u * i + 1u], rgb[3u * i + 2u]);
}

fn bilinear_rgb(x: i32, y: i32) -> vec3<f32> {
    return vec3<f32>(bilinear(x, y, 0u), bilinear(x, y, 1u), bilinear(x, y, 2u));
}

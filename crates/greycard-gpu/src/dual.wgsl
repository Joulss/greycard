// The dual demosaic's blend mask on the GPU: `blend_mask` in
// `greycard_core::develop::dual`, after RawTherapee's `buildBlendMask`
// (Ingo Weyrich, GPL-3.0-or-later). The lightness of the detail
// result into `plane_a`, the sigmoid of its local contrast into
// `plane_b`, and the mask's blur through `plane_d` and back.

const L_SCALE: f32 = 327.68;
const CONTRAST_SCALE: f32 = 0.0625 / 327.68;
const BLUR_RADIUS: i32 = 6;

// The cube root to the last bit or so: the power's estimate and a
// Newton step.
fn cube_root(v: f32) -> f32 {
    let r = pow(v, 1.0 / 3.0);
    return r - (r * r * r - v) / (3.0 * r * r);
}

@compute @workgroup_size(16, 16)
fn dual_lightness(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let i = at(i32(id.x), i32(id.y));
    let px = get_rgb(i);
    let y = 0.212671 * px.x + 0.715160 * px.y + 0.072169 * px.z;
    var f: f32;
    if y > 216.0 / 24389.0 {
        f = cube_root(y);
    } else {
        f = (24389.0 / 27.0 * y + 16.0) / 116.0;
    }
    plane_a[i] = (116.0 * f - 16.0) * L_SCALE;
}

fn sq(v: f32) -> f32 {
    return v * v;
}

fn lightness_at(x: i32, y: i32) -> f32 {
    return plane_a[at(x, y)];
}

// The threshold at lightness `l`, from the table: the reference's
// rounding to a unit of L*, half away from zero (`round` in WGSL is
// half to even, and adding a half before the floor rounds where the
// sum does). Below zero it clamps to the first entry either way.
fn threshold_at(l: f32) -> f32 {
    let v = l / L_SCALE;
    var r = floor(v);
    if v - r >= 0.5 {
        r += 1.0;
    }
    let i = clamp(r, 0.0, 100.0);
    return consts[7u + u32(i)];
}

fn blend_factor(contrast: f32, threshold: f32) -> f32 {
    let x = -16.0 + (16.0 / threshold) * contrast;
    return 0.5 * (1.0 + x / sqrt(1.0 + x * x));
}

// The sigmoid of the local contrast, two pixels in from the edge and
// copied out from the nearest such pixel, into `plane_b`.
@compute @workgroup_size(16, 16)
fn dual_mask(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = clamp(i32(id.x), 2, band_w() - 3);
    let y = clamp(i32(id.y), 2, band_h() - 3);
    let t = threshold_at(lightness_at(x, y));
    var b = 1.0;
    if t > 0.0 {
        let contrast = sqrt(
            sq(lightness_at(x + 1, y) - lightness_at(x - 1, y))
                + sq(lightness_at(x, y + 1) - lightness_at(x, y - 1))
                + sq(lightness_at(x + 2, y) - lightness_at(x - 2, y))
                + sq(lightness_at(x, y + 2) - lightness_at(x, y - 2)),
        ) * CONTRAST_SCALE;
        b = blend_factor(contrast, t);
    }
    plane_b[at(i32(id.x), i32(id.y))] = b;
}

// The mask's blur along the rows, `plane_b` into `plane_d`, the taps
// clamped at the edges, in the reference's order.
@compute @workgroup_size(16, 16)
fn dual_blur_rows(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    let last = band_w() - 1;
    var acc = consts[0] * plane_b[at(x, y)];
    for (var k = 1; k <= BLUR_RADIUS; k++) {
        acc += consts[k] * (plane_b[at(clamp(x - k, 0, last), y)]
            + plane_b[at(clamp(x + k, 0, last), y)]);
    }
    plane_d[at(x, y)] = acc;
}

// The blur down the columns, `plane_d` back into `plane_b`.
@compute @workgroup_size(16, 16)
fn dual_blur_columns(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    let last = band_h() - 1;
    var acc = consts[0] * plane_d[at(x, y)];
    for (var k = 1; k <= BLUR_RADIUS; k++) {
        acc += consts[k] * (plane_d[at(x, clamp(y - k, 0, last))]
            + plane_d[at(x, clamp(y + k, 0, last))]);
    }
    plane_b[at(x, y)] = acc;
}

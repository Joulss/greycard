// RCD on the GPU: `greycard_core::develop::rcd`, Luis Sanz
// Rodríguez's Ratio Corrected Demosaicing (release 2.3,
// GPL-3.0-or-later), a kernel a pass. Each takes the reference's
// operations in the reference's order; see `demosaic.rs` for where
// the two can still differ.
//
// Planes: `plane_a` the vertical/horizontal discrimination, `plane_b`
// the low-pass filter and then the diagonal discrimination,
// `plane_c` the green, `plane_d` the missing chroma at red and blue
// sites; `rgb` the result.

const RCD_BORDER: i32 = 4;

// Nine samples centered on (x, y), (sx, sy) apart, as the reference's
// quadratic form reads them.
fn direction_stat(x: i32, y: i32, sx: i32, sy: i32) -> f32 {
    let m4 = s(x - 4 * sx, y - 4 * sy);
    let m3 = s(x - 3 * sx, y - 3 * sy);
    let m2 = s(x - 2 * sx, y - 2 * sy);
    let m1 = s(x - sx, y - sy);
    let c = s(x, y);
    let p1 = s(x + sx, y + sy);
    let p2 = s(x + 2 * sx, y + 2 * sy);
    let p3 = s(x + 3 * sx, y + 3 * sy);
    let p4 = s(x + 4 * sx, y + 4 * sy);
    let v = -18.0 * c * m1 - 18.0 * c * p1 - 36.0 * c * m2 - 36.0 * c * p2
        + 18.0 * c * m3
        + 18.0 * c * p3
        - 2.0 * c * m4
        - 2.0 * c * p4
        + 38.0 * c * c
        - 70.0 * m1 * p1
        - 12.0 * m1 * m2
        + 24.0 * m1 * p2
        - 38.0 * m1 * m3
        + 16.0 * m1 * p3
        + 12.0 * m1 * m4
        - 6.0 * m1 * p4
        + 46.0 * m1 * m1
        + 24.0 * p1 * m2
        - 12.0 * p1 * p2
        + 16.0 * p1 * m3
        - 38.0 * p1 * p3
        - 6.0 * p1 * m4
        + 12.0 * p1 * p4
        + 46.0 * p1 * p1
        + 14.0 * m2 * p2
        - 12.0 * m2 * p3
        - 2.0 * m2 * m4
        + 2.0 * m2 * p4
        + 11.0 * m2 * m2
        - 12.0 * p2 * m3
        + 2.0 * p2 * m4
        - 2.0 * p2 * p4
        + 11.0 * p2 * p2
        + 2.0 * m3 * p3
        - 6.0 * m3 * m4
        + 10.0 * m3 * m3
        - 6.0 * p3 * p4
        + 10.0 * p3 * p3
        + m4 * m4
        + p4 * p4;
    return max(v, EPS_SQ);
}

// The reference's refinement: the mean of the four diagonal neighbors
// where it is the more decisive.
fn refine(central: f32, nw: f32, ne: f32, sw: f32, se: f32) -> f32 {
    let neighborhood = 0.25 * (nw + ne + sw + se);
    if abs(0.5 - central) < abs(0.5 - neighborhood) {
        return neighborhood;
    }
    return central;
}

fn refined_a(x: i32, y: i32) -> f32 {
    return refine(
        plane_a[at(x, y)],
        plane_a[at(x - 1, y - 1)],
        plane_a[at(x + 1, y - 1)],
        plane_a[at(x - 1, y + 1)],
        plane_a[at(x + 1, y + 1)],
    );
}

fn refined_b(x: i32, y: i32) -> f32 {
    return refine(
        plane_b[at(x, y)],
        plane_b[at(x - 1, y - 1)],
        plane_b[at(x + 1, y - 1)],
        plane_b[at(x - 1, y + 1)],
        plane_b[at(x + 1, y + 1)],
    );
}

// Pass 1: the vertical/horizontal discrimination, into `plane_a`.
@compute @workgroup_size(16, 16)
fn rcd_vh(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    var out = 0.5;
    if interior(x, y, RCD_BORDER) {
        let v = direction_stat(x, y, 0, 1);
        let h = direction_stat(x, y, 1, 0);
        out = v / (v + h);
    }
    plane_a[at(x, y)] = out;
}

// Pass 2: the low-pass filter at red and blue sites, into `plane_b`.
@compute @workgroup_size(16, 16)
fn rcd_lpf(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    var out = 0.0;
    if interior(x, y, 2) && !is_green(x, y) {
        out = 0.25 * s(x, y)
            + 0.125 * (s(x, y - 1) + s(x, y + 1) + s(x - 1, y) + s(x + 1, y))
            + 0.0625 * (s(x - 1, y - 1) + s(x + 1, y - 1) + s(x - 1, y + 1) + s(x + 1, y + 1));
    }
    plane_b[at(x, y)] = out;
}

// A neighbor's green scaled by how the low-pass image changes across
// it.
fn ratio_estimate(g: f32, l0: f32, far: f32) -> f32 {
    return g * (1.0 + (l0 - far) / (EPS + l0 + far));
}

// Pass 3: the green at red and blue sites, into `plane_c`; the
// bilinear green elsewhere.
@compute @workgroup_size(16, 16)
fn rcd_green(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    if !interior(x, y, RCD_BORDER) || is_green(x, y) {
        plane_c[at(x, y)] = bilinear(x, y, 1u);
        return;
    }
    let disc = refined_a(x, y);
    let c0 = s(x, y);
    let n_grad = EPS
        + abs(s(x, y - 1) - s(x, y + 1))
        + abs(c0 - s(x, y - 2))
        + abs(s(x, y - 1) - s(x, y - 3))
        + abs(s(x, y - 2) - s(x, y - 4));
    let s_grad = EPS
        + abs(s(x, y + 1) - s(x, y - 1))
        + abs(c0 - s(x, y + 2))
        + abs(s(x, y + 1) - s(x, y + 3))
        + abs(s(x, y + 2) - s(x, y + 4));
    let w_grad = EPS
        + abs(s(x - 1, y) - s(x + 1, y))
        + abs(c0 - s(x - 2, y))
        + abs(s(x - 1, y) - s(x - 3, y))
        + abs(s(x - 2, y) - s(x - 4, y));
    let e_grad = EPS
        + abs(s(x + 1, y) - s(x - 1, y))
        + abs(c0 - s(x + 2, y))
        + abs(s(x + 1, y) - s(x + 3, y))
        + abs(s(x + 2, y) - s(x + 4, y));
    let l0 = plane_b[at(x, y)];
    let n_est = ratio_estimate(s(x, y - 1), l0, plane_b[at(x, y - 2)]);
    let s_est = ratio_estimate(s(x, y + 1), l0, plane_b[at(x, y + 2)]);
    let w_est = ratio_estimate(s(x - 1, y), l0, plane_b[at(x - 2, y)]);
    let e_est = ratio_estimate(s(x + 1, y), l0, plane_b[at(x + 2, y)]);
    let v_est = (s_grad * n_est + n_grad * s_est) / (n_grad + s_grad);
    let h_est = (w_grad * e_est + e_grad * w_est) / (e_grad + w_grad);
    plane_c[at(x, y)] = clamp(disc * h_est + (1.0 - disc) * v_est, 0.0, params.ceiling);
}

// Pass 4a: the diagonal discrimination at red and blue sites, into
// `plane_b`, the low-pass filter done with.
@compute @workgroup_size(16, 16)
fn rcd_pq(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    var out = 0.5;
    if interior(x, y, RCD_BORDER) && !is_green(x, y) {
        let p = direction_stat(x, y, 1, 1);
        let q = direction_stat(x, y, -1, 1);
        out = p / (p + q);
    }
    plane_b[at(x, y)] = out;
}

// Pass 4b: the missing chroma at red and blue sites, into `plane_d`:
// blue at red, red at blue.
@compute @workgroup_size(16, 16)
fn rcd_other(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    if is_green(x, y) {
        plane_d[at(x, y)] = 0.0;
        return;
    }
    if !interior(x, y, RCD_BORDER) {
        plane_d[at(x, y)] = bilinear(x, y, 2u - color(x, y));
        return;
    }
    let disc = refined_b(x, y);
    let g0 = plane_c[at(x, y)];
    let nw_grad = EPS
        + abs(s(x - 1, y - 1) - s(x + 1, y + 1))
        + abs(s(x - 1, y - 1) - s(x - 3, y - 3))
        + abs(g0 - plane_c[at(x - 2, y - 2)]);
    let ne_grad = EPS
        + abs(s(x + 1, y - 1) - s(x - 1, y + 1))
        + abs(s(x + 1, y - 1) - s(x + 3, y - 3))
        + abs(g0 - plane_c[at(x + 2, y - 2)]);
    let sw_grad = EPS
        + abs(s(x - 1, y + 1) - s(x + 1, y - 1))
        + abs(s(x - 1, y + 1) - s(x - 3, y + 3))
        + abs(g0 - plane_c[at(x - 2, y + 2)]);
    let se_grad = EPS
        + abs(s(x + 1, y + 1) - s(x - 1, y - 1))
        + abs(s(x + 1, y + 1) - s(x + 3, y + 3))
        + abs(g0 - plane_c[at(x + 2, y + 2)]);
    let nw_est = s(x - 1, y - 1) - plane_c[at(x - 1, y - 1)];
    let ne_est = s(x + 1, y - 1) - plane_c[at(x + 1, y - 1)];
    let sw_est = s(x - 1, y + 1) - plane_c[at(x - 1, y + 1)];
    let se_est = s(x + 1, y + 1) - plane_c[at(x + 1, y + 1)];
    let p_est = (nw_grad * se_est + se_grad * nw_est) / (nw_grad + se_grad);
    let q_est = (ne_grad * sw_est + sw_grad * ne_est) / (ne_grad + sw_grad);
    plane_d[at(x, y)] = clamp(g0 + (1.0 - disc) * p_est + disc * q_est, 0.0, params.ceiling);
}

// A red-or-blue site's value of color `c`: its own sample where that
// is its color, else pass 4b's.
fn chroma_of(x: i32, y: i32, c: u32) -> f32 {
    if color(x, y) == c {
        return s(x, y);
    }
    return plane_d[at(x, y)];
}

// Pass 4c: red and blue at green sites, and the assembly into `rgb`;
// the bilinear result on the border.
@compute @workgroup_size(16, 16)
fn rcd_rgb(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    let i = at(x, y);
    if !interior(x, y, RCD_BORDER) {
        put_rgb(i, bilinear_rgb(x, y));
        return;
    }
    if !is_green(x, y) {
        var px = vec3<f32>(0.0);
        let own = color(x, y);
        px[own] = s(x, y);
        px[1] = plane_c[i];
        px[2u - own] = plane_d[i];
        put_rgb(i, px);
        return;
    }
    var px = vec3<f32>(0.0, s(x, y), 0.0);
    let disc = refined_a(x, y);
    let g0 = plane_c[i];
    for (var c = 0u; c <= 2u; c += 2u) {
        let n_grad = EPS
            + abs(g0 - plane_c[at(x, y - 2)])
            + abs(chroma_of(x, y - 1, c) - chroma_of(x, y + 1, c))
            + abs(chroma_of(x, y - 1, c) - chroma_of(x, y - 3, c));
        let s_grad = EPS
            + abs(g0 - plane_c[at(x, y + 2)])
            + abs(chroma_of(x, y + 1, c) - chroma_of(x, y - 1, c))
            + abs(chroma_of(x, y + 1, c) - chroma_of(x, y + 3, c));
        let w_grad = EPS
            + abs(g0 - plane_c[at(x - 2, y)])
            + abs(chroma_of(x - 1, y, c) - chroma_of(x + 1, y, c))
            + abs(chroma_of(x - 1, y, c) - chroma_of(x - 3, y, c));
        let e_grad = EPS
            + abs(g0 - plane_c[at(x + 2, y)])
            + abs(chroma_of(x + 1, y, c) - chroma_of(x - 1, y, c))
            + abs(chroma_of(x + 1, y, c) - chroma_of(x + 3, y, c));
        let n_est = chroma_of(x, y - 1, c) - plane_c[at(x, y - 1)];
        let s_est = chroma_of(x, y + 1, c) - plane_c[at(x, y + 1)];
        let w_est = chroma_of(x - 1, y, c) - plane_c[at(x - 1, y)];
        let e_est = chroma_of(x + 1, y, c) - plane_c[at(x + 1, y)];
        let v_est = (n_grad * s_est + s_grad * n_est) / (n_grad + s_grad);
        let h_est = (e_grad * w_est + w_grad * e_est) / (e_grad + w_grad);
        px[c] = clamp(g0 + (1.0 - disc) * v_est + disc * h_est, 0.0, params.ceiling);
    }
    put_rgb(i, px);
}

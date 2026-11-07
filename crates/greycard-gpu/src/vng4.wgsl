// VNG4 on the GPU: `greycard_core::develop::vng4`, RawTherapee's VNG4
// (Ingo Weyrich, after dcraw's VNG by Dave Coffin and Chang, Cheung
// and Pau; GPL-3.0-or-later). The gradient terms of each phase come
// from the reference's own table, in its order; see `demosaic.rs` for
// where the two can differ.
//
// Planes: `plane_a` and `plane_d` the interpolated greens of red rows
// and of blue rows, `plane_c` the VNG green; `rgb` the result, or the
// detail result it is blended into.

const VNG_BORDER: i32 = 3;

fn fc4(x: i32, y: i32) -> u32 {
    return params.fc4[phase(x, y)];
}

// Color `c` at a site one in from the edge: the sample where the site
// has it, else the 3x3 mean with cardinal neighbors weighted 2.
fn vng_plane(c: u32, x: i32, y: i32) -> f32 {
    if fc4(x, y) == c {
        return s(x, y);
    }
    var sum = 0.0;
    var weight = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            if (dy == 0 && dx == 0) || fc4(x + dx, y + dy) != c {
                continue;
            }
            let w = f32(1u << (u32(dy == 0) + u32(dx == 0)));
            sum += w * s(x + dx, y + dy);
            weight += w;
        }
    }
    return sum / weight;
}

// The two green planes, where the interior reads them.
@compute @workgroup_size(16, 16)
fn vng_planes(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    var one = 0.0;
    var three = 0.0;
    if interior(x, y, 1) {
        one = vng_plane(1u, x, y);
        three = vng_plane(3u, x, y);
    }
    plane_a[at(x, y)] = one;
    plane_d[at(x, y)] = three;
}

fn green_plane(c: u32, x: i32, y: i32) -> f32 {
    if c == 1u {
        return plane_a[at(x, y)];
    }
    return plane_d[at(x, y)];
}

const CHOOD = array<vec2<i32>, 8>(
    vec2<i32>(-1, -1),
    vec2<i32>(0, -1),
    vec2<i32>(1, -1),
    vec2<i32>(1, 0),
    vec2<i32>(1, 1),
    vec2<i32>(0, 1),
    vec2<i32>(-1, 1),
    vec2<i32>(-1, 0),
);

// The VNG green at an interior site: the gradients summed term by term
// in the reference's order, the directions under the threshold, and
// the mean color difference over them.
fn vng_green_at(x: i32, y: i32) -> f32 {
    var gval = array<f32, 8>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let p = phase(x, y);
    let start = params.term_start[p];
    let count = params.term_count[p];
    for (var k = 0u; k < count; k++) {
        let t = terms[start + k];
        let diff = abs(s(x + t.d1.y, y + t.d1.x) - s(x + t.d2.y, y + t.d2.x)) * t.weight;
        for (var g = 0u; g < 8u; g++) {
            if ((t.grads >> g) & 1u) != 0u {
                gval[g] += diff;
            }
        }
    }
    var lo = gval[0];
    var hi = gval[0];
    for (var g = 1u; g < 8u; g++) {
        lo = min(lo, gval[g]);
        hi = max(hi, gval[g]);
    }
    let thold = lo + hi * 0.5;
    let own = s(x, y);
    let c = fc4(x, y);
    let green = (c & 1u) == 1u;
    var sum0 = 0.0;
    var sum1 = 0.0;
    var num = 0u;
    for (var g = 0u; g < 8u; g++) {
        if gval[g] > thold {
            continue;
        }
        let d = CHOOD[g];
        let nx = x + d.x;
        let ny = y + d.y;
        sum0 += own + s(x + 2 * d.x, y + 2 * d.y);
        if green {
            sum1 += green_plane(c ^ 2u, nx, ny);
        } else {
            sum1 += green_plane(1u, nx, ny) + green_plane(3u, nx, ny);
        }
        num++;
    }
    if green {
        sum0 *= 0.5;
    }
    return max(own + (sum1 - sum0) / f32(2u * num), 0.0);
}

// The green at every site, into `plane_c`: the gradients' where they
// fit, bilinear on the border.
@compute @workgroup_size(16, 16)
fn vng_green(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    var g: f32;
    if interior(x, y, 2) {
        g = vng_green_at(x, y);
    } else {
        g = bilinear(x, y, 1u);
    }
    plane_c[at(x, y)] = g;
}

fn vng_raw_less_green(x: i32, y: i32) -> f32 {
    return s(x, y) - plane_c[at(x, y)];
}

// The VNG4 result at a site: color differences against the VNG green,
// bilinear on the border.
fn vng_rgb_at(x: i32, y: i32) -> vec3<f32> {
    if !interior(x, y, VNG_BORDER) {
        return bilinear_rgb(x, y);
    }
    var row_color = 0u;
    if fc4(0, y) == 2u || fc4(1, y) == 2u {
        row_color = 2u;
    }
    let other_color = 2u - row_color;
    let g = plane_c[at(x, y)];
    var own: f32;
    var other: f32;
    if (fc4(x, y) & 1u) == 0u {
        var rb = 0.0;
        rb += vng_raw_less_green(x - 1, y - 1);
        rb += vng_raw_less_green(x + 1, y - 1);
        rb += vng_raw_less_green(x - 1, y + 1);
        rb += vng_raw_less_green(x + 1, y + 1);
        own = s(x, y);
        other = max(g + rb * 0.25, 0.0);
    } else {
        let h = g
            + (s(x - 1, y) - plane_c[at(x - 1, y)] + s(x + 1, y) - plane_c[at(x + 1, y)])
            * 0.5;
        let v = g
            + (s(x, y - 1) - plane_c[at(x, y - 1)] + s(x, y + 1) - plane_c[at(x, y + 1)])
            * 0.5;
        own = max(h, 0.0);
        other = max(v, 0.0);
    }
    var px = vec3<f32>(0.0);
    px[row_color] = min(own, params.ceiling);
    px[1] = min(g, params.ceiling);
    px[other_color] = min(other, params.ceiling);
    return px;
}

// The VNG4 result into `rgb`, or, for the dual, blended into the
// detail result there by the mask in `plane_b`: `b * (detail - vng)
// + vng`, the reference's `intp`.
@compute @workgroup_size(16, 16)
fn vng_rgb(@builtin(global_invocation_id) id: vec3<u32>) {
    if !inside(id) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    let i = at(x, y);
    let vng = vng_rgb_at(x, y);
    if params.mode == 2u {
        let b = plane_b[i];
        put_rgb(i, b * (get_rgb(i) - vng) + vng);
    } else {
        put_rgb(i, vng);
    }
}

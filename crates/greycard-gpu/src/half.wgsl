// A store to an rgba16float texture converts each f32 to a half float
// in a rounding the device chooses, and NVIDIA and lavapipe both round
// toward zero; the CPU's halves round to nearest, ties to even. So
// every op rounds its result here first, to a value the half float
// holds exactly, which the store then keeps whatever its rounding.
// Integer arithmetic on the bits, so that no device's float rounding,
// fused or fast, comes into it. Prepended to each shader that stores
// half floats.

// The half float nearest `v`, ties to even, as an f32: past the
// largest finite half (65504) by half its ulp or more an infinity of
// the sign, a NaN as it is, under the least subnormal's half a zero of
// the sign.
fn half_nearest(v: f32) -> f32 {
    let bits = bitcast<u32>(v);
    let sign = bits & 0x80000000u;
    let a = bits & 0x7fffffffu;
    if a > 0x7f800000u {
        return v;
    }
    if a >= 0x477ff000u {
        return bitcast<f32>(sign | 0x7f800000u);
    }
    if a >= 0x38800000u {
        // A normal half: drop the thirteen mantissa bits it has no
        // room for, rounded; a carry steps the exponent, as it should.
        let r = (a + 0xfffu + ((a >> 13u) & 1u)) & 0xffffe000u;
        return bitcast<f32>(sign | r);
    }
    // A subnormal half, a multiple of 2^-24: the mantissa with its
    // leading one, shifted down to those units, rounded.
    let e = a >> 23u;
    if e < 102u {
        return bitcast<f32>(sign);
    }
    let m = (a & 0x7fffffu) | 0x800000u;
    let s = 126u - e;
    let q = (m + (1u << (s - 1u)) - 1u + ((m >> s) & 1u)) >> s;
    return bitcast<f32>(sign | bitcast<u32>(f32(q) * 0x1p-24f));
}

fn halves_nearest(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(half_nearest(v.x), half_nearest(v.y), half_nearest(v.z), half_nearest(v.w));
}

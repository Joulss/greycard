//! A picture past the device's textures, reduced to fit them.
//!
//! A texture can be no wider or taller than the device's
//! `max_texture_dimension_2d`: 16384 on every Apple GPU and on
//! lavapipe, 32768 on a recent discrete card, 8192 at the least wgpu
//! guarantees. A stitched panorama or a large JPEG can be past it, and
//! a texture asked for past it fails validation, which the window's
//! handler takes as a fault and ends the process on. So whatever the
//! viewport puts on the device from a picture's size is reduced here
//! first, by the smallest whole factor that fits, each texel the mean
//! of the square of pixels it stands for; the picture's geometry stays
//! in its own full pixels, and the shader reads the reduced texture
//! where those land in it.

use rayon::prelude::*;

/// The smallest whole factor that brings `width` by `height` within
/// `most` a side: 1 for a picture that fits as it is. The reduced
/// sides are `div_ceil` of the factor, a last texel standing for the
/// pixels left over.
pub fn factor(width: u32, height: u32, most: u32) -> u32 {
    let most = most.max(1);
    let long = width.max(height);
    let mut k = long.div_ceil(most).max(1);
    // `div_ceil` of the long side by `most` already fits it; the loop
    // only guards the rounding.
    while long.div_ceil(k) > most {
        k += 1;
    }
    k
}

/// `width` by `height` reduced by `k` each way.
pub fn reduced_size(width: u32, height: u32, k: u32) -> (u32, u32) {
    let k = k.max(1);
    (width.div_ceil(k).max(1), height.div_ceil(k).max(1))
}

/// A plane of `C` channels a texel, `width` texels a row, reduced by
/// `k` each way: texel i the mean of the box of pixels [k i, k i + k)
/// each way, read and written through `get` and `put`. A box the
/// picture's far edge cuts is padded with the edge pixel, as the
/// sampler clamps: every texel stands for a whole box, its center at
/// the box's, so a position in the picture's pixels divided by `k` is
/// the position in the texture's, at every texel (the shader's
/// `to_texture`). A row of the result at a time, in parallel.
pub fn plane<T, const C: usize>(
    data: &[T],
    width: usize,
    height: usize,
    k: usize,
    get: impl Fn(T) -> f32 + Sync,
    put: impl Fn(f32) -> T + Sync,
) -> Vec<T>
where
    T: Copy + Send + Sync + Default,
{
    let k = k.max(1);
    let (w, h) = (width.div_ceil(k), height.div_ceil(k));
    let mut out = vec![T::default(); w * h * C];
    out.par_chunks_mut(w * C).enumerate().for_each(|(ry, row)| {
        let mut acc = vec![0.0f32; w * C];
        for y in (ry * k..(ry + 1) * k).map(|y| y.min(height - 1)) {
            let line = &data[y * width * C..(y + 1) * width * C];
            for (rx, a) in acc.chunks_mut(C).enumerate() {
                for x in (rx * k..(rx + 1) * k).map(|x| x.min(width - 1)) {
                    for c in 0..C {
                        a[c] += get(line[x * C + c]);
                    }
                }
            }
        }
        let n = (k * k) as f32;
        for (o, a) in row.chunks_mut(C).zip(acc.chunks(C)) {
            for c in 0..C {
                o[c] = put(a[c] / n);
            }
        }
    });
    out
}

/// Half floats of `C` channels, reduced by `k`.
pub fn halves<const C: usize>(
    data: &[half::f16],
    width: usize,
    height: usize,
    k: usize,
) -> Vec<half::f16> {
    plane::<half::f16, C>(data, width, height, k, half::f16::to_f32, |v| {
        half::f16::from_f32(v)
    })
}

/// Bytes of `C` channels, reduced by `k`, rounded to the nearest.
pub fn bytes<const C: usize>(data: &[u8], width: usize, height: usize, k: usize) -> Vec<u8> {
    plane::<u8, C>(data, width, height, k, f32::from, |v| {
        v.round().clamp(0.0, 255.0) as u8
    })
}

/// A plane of one byte a texel that covers its picture edge to edge
/// (a mask's raster, read by `uv * size`), brought to `nw` by `nh`
/// texels that cover it edge to edge too: texel j the mean of the
/// texels i with `i * nw / width == j` each way. Its centers land
/// within half a texel of the source's at the same place, at every
/// texel, which a box of whole factors would not where the sides are
/// not multiples of it.
pub fn bytes_spread(data: &[u8], width: usize, height: usize, nw: usize, nh: usize) -> Vec<u8> {
    let (nw, nh) = (nw.clamp(1, width), nh.clamp(1, height));
    let mut out = vec![0u8; nw * nh];
    out.par_chunks_mut(nw).enumerate().for_each(|(j, row)| {
        let (y0, y1) = ((j * height).div_ceil(nh), ((j + 1) * height).div_ceil(nh));
        let mut acc = vec![(0u32, 0u32); nw];
        for y in y0..y1 {
            for (x, &v) in data[y * width..(y + 1) * width].iter().enumerate() {
                let a = &mut acc[x * nw / width];
                a.0 += u32::from(v);
                a.1 += 1;
            }
        }
        for (o, (sum, n)) in row.iter_mut().zip(acc) {
            *o = ((sum as f32 / n.max(1) as f32).round()) as u8;
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The factor is the smallest that fits, either side the longer,
    /// a picture at the limit is not reduced, and one a pixel over is
    /// halved.
    #[test]
    fn the_factor_is_the_smallest_that_fits() {
        assert_eq!(factor(16384, 16384, 16384), 1);
        assert_eq!(factor(16385, 10, 16384), 2);
        assert_eq!(factor(10, 16385, 16384), 2);
        assert_eq!(factor(18432, 10368, 16384), 2);
        assert_eq!(factor(17000, 400, 16384), 2);
        assert_eq!(factor(400, 17000, 16384), 2);
        assert_eq!(factor(32769, 5, 16384), 3);
        assert_eq!(factor(1, 1, 16384), 1);
        // A tiny limit and odd sides.
        for (w, h, most) in [(257, 3, 256), (513, 511, 256), (7, 1001, 13), (999, 999, 1)] {
            let k = factor(w, h, most);
            let (rw, rh) = reduced_size(w, h, k);
            assert!(rw <= most && rh <= most, "{w}x{h} at {most}: {k}");
            if k > 1 {
                let (pw, ph) = reduced_size(w, h, k - 1);
                assert!(
                    pw > most || ph > most,
                    "{w}x{h} at {most}: {k} not the least"
                );
            }
        }
    }

    /// Each texel is the mean of its box, a box the edge cuts padded
    /// with the edge pixel.
    #[test]
    fn a_texel_is_the_mean_of_its_box() {
        let (w, h) = (5usize, 3usize);
        let data: Vec<u8> = (0..w * h).map(|i| i as u8 * 10).collect();
        let r = bytes::<1>(&data, w, h, 2);
        assert_eq!(r.len(), 3 * 2);
        // (0 + 10 + 50 + 60) / 4; the last column (40 + 40 + 90 + 90)
        // / 4; the last row (100 + 110 + 100 + 110) / 4; and 140 four
        // times.
        assert_eq!(r, vec![30, 50, 65, 105, 125, 140]);
        // Where the padding weighs: a box of 3 with one pixel past
        // the edge counts the edge twice.
        let r = bytes::<1>(&[0, 30, 90, 0], 4, 1, 3);
        assert_eq!(r, vec![40, 0]);
        let halves4: Vec<half::f16> = data
            .iter()
            .flat_map(|&v| [half::f16::from_f32(f32::from(v)); 4])
            .collect();
        let r4 = halves::<4>(&halves4, w, h, 2);
        let firsts: Vec<f32> = r4.chunks(4).map(|t| t[0].to_f32()).collect();
        assert_eq!(firsts, vec![30.0, 50.0, 65.0, 105.0, 125.0, 140.0]);
    }

    /// Spread edge to edge, each source texel goes to the bin its
    /// place falls in, and a lone bright texel's bin center stays
    /// within half a source texel of it, at every place.
    #[test]
    fn a_spread_keeps_each_texel_in_its_place() {
        let r = bytes_spread(&[10, 20, 30, 40, 50], 5, 1, 2, 1);
        // 0*2/5 = 0, 1 -> 0, 2 -> 0, 3 -> 1, 4 -> 1.
        assert_eq!(r, vec![20, 45]);
        for (w, n) in [(7usize, 3usize), (701, 234), (12289, 6145), (999, 250)] {
            for i in [0, w / 3, w / 2, w - 1] {
                let mut d = vec![0u8; w];
                d[i] = 255;
                let r = bytes_spread(&d, w, 1, n, 1);
                let j = r.iter().position(|&v| v > 0).unwrap();
                let at = (j as f32 + 0.5) * w as f32 / n as f32;
                let bin = (0..w).filter(|x| x * n / w == j).count() as f32;
                assert!(
                    (at - (i as f32 + 0.5)).abs() <= bin / 2.0 + 0.5,
                    "{w} to {n}: texel {i} at {at}"
                );
            }
        }
    }
}

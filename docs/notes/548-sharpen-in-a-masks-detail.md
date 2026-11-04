# §548. Sharpen in a mask's Detail

*2026-11-04*

Asked three times (pixls 2026-10-24, discussion 59 twice): a mask's
DETAIL gains **Sharpen**, beside Texture, Clarity, Structure, and
Dehaze.

**The delta plane.** The capture sharpen is a deconvolution, not an
amount on a band, so it cannot join the bands of §287 as they are. It
joins as a plane of its own: the sharpen's delta, `log2(new / old)` of
the luminance on its pedestal, in stops, made where the develop
sharpens (`sharpen_with_delta`, and on the GPU a `delta` pass beside
the apply, read back). A mask's Sharpen, -1 to 1, puts `k · delta` on
the pixel by the sharpen's own rule, every channel scaled about the
pedestal's negative (`apply_delta`), so -1 is the sharpen taken off and
+1 the sharpen once more. The radius, iterations, and threshold are the
global's. A mask's Sharpen is a share of the sharpen, never less than
none: with the global Sharpen off, the summed share under the masks is
held at zero from below (`DetailMeta::sharpened`, the sign of the
pedestal in the shader), so a negative one does nothing, and the
develop runs the deconvolution for the delta alone, the picture left
unsharpened (`Delta::Only`; `keep_half` on the GPU), only when some
enabled mask's Sharpen is positive. -100 under a negative-only edit
then costs nothing and is the unsharpened export to the bit. The plane
is made only when an enabled mask asks; an edit without one runs and
keys as before. `Detail.sharpen` is left out of a sidecar at zero, as
`structure` is; no schema bump. The reduced develop skips it with the
global sharpen.

**The ordering.** The delta goes on first at the finish, before the
mask's other Detail: it undoes or repeats the develop's last op. At -1
over the sharpen, on a 24 MP frame, a pixel's luminance is the
unsharpened develop's to 6.2e-5 at most (p99.9 1.5e-5; the sharpen
itself moved it by up to 0.27); through the export, 1 level of 65535.
Sharpen +1 over an unsharpened frame is the global sharpen, ΔE 0.000.
The difference the order makes: a mask's Texture -0.5 with Sharpen +1,
global sharpen off, against the global Texture -0.5 with the sharpen
on, which sharpens the textured picture: Oklab ΔE ×100 mean 0.116, p99
0.72 over the whole frame, under §302's own 1.

**GPU against CPU.** The delta is the reference's to 4e-6 stops on the
synthetic picture, sharpened or not, and the picture untouched without
the sharpen. The shader's Sharpen is the CPU finish's to two levels,
alone and with the bands, and the random parity test draws it.

**Costs.** The plane is f16, 2 bytes a pixel: 48 MB at 24 MP, 90 MB at
45 MP in memory, the same as an `R16Float` texture in the viewport
(binding 14). On the GPU's path the delta is an `R32Float` texture
while it is made (96 MB, 180 MB) and read back once a develop. The
export carries it through the geometry as a one-channel f32 plane, as
Structure's band. Develop time, 24 MP, release, four threads, best of
three, a white moved:

| Path | Global Sharpen | Without | With a mask's Sharpen |
|---|---|---|---|
| GPU | on | 1.71 s | 1.70 s |
| GPU | off | 1.53 s | 1.65 s |
| CPU | on | 2.54 s | 2.53 s |
| CPU | off | 1.77 s | 2.50 s |

With the sharpen on, the delta costs nothing visible; with it off, a
mask's positive Sharpen pays the deconvolution, 0.12 s on the GPU and
0.73 s on the CPU.

**The kept delta.** The worker keeps the last delta for the kept
pictures. A walk that reaches the sharpen replaces it with its own, or
with none when it makes none, so a delta of an earlier picture is never
read beside a later one.

**Left open.** The thumbnails reduce the delta with the picture, so a
mask's Sharpen there is as faint as the global's. The droppers' `pick`
ignores it, as it ignores the other Detail.

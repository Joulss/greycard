"""Compare two develops of one frame, pixel for pixel, in stops.

    python compare.py FP32.tif FP16.tif [MORE_FP32.tif MORE_FP16.tif ...]

For the fp16 denoisers against the fp32 ones (§481, §489, §508): each
pair is the same raw developed by the CLI with the two files,

    greycard -v develop RAW --ai-denoise denoise-v20.onnx      -o a.tif
    greycard -v develop RAW --ai-denoise denoise-v20-fp16.onnx -o b.tif

the default output, a 16-bit linear Rec.2020 TIFF. Naming the files by
path keeps the denoise cache out of it. The log says which provider
ran and the denoiser's time: the default build runs WebGPU (racing
Core ML on a Mac), a build with the `cuda` feature CUDA first, so the
two builds compare the two providers.

Printed for each pair: the PSNR, and over four bands of luminance
(deep shadows under 0.002 linear, shadows to 0.01, mids to 0.1,
highlights above) the share of pixels, the mean and 99.9th percentile
of the luminance difference in stops, and each channel's signed mean,
which shows a lift where the mean alone shows only a spread. On Core
ML on an M5 the fp16 files gave a PSNR of 74 to 83 dB, a 99.9th
percentile under 0.01 stops outside the deep shadows, and a lift of
about +0.001 stops; the deep shadows' figure, about 0.01 to 0.02, is
one step of the 16-bit file there, the measure's floor.

Needs numpy and tifffile (`pip install numpy tifffile imagecodecs`;
imagecodecs for the CLI's compressed TIFFs).
"""

import sys

import numpy as np
import tifffile

BANDS = [
    ("deep shadows", 0.0, 0.002),
    ("shadows", 0.002, 0.01),
    ("mids", 0.01, 0.1),
    ("highlights", 0.1, np.inf),
]
# Rec.2020 luminance.
WEIGHTS = np.array([0.2627, 0.6780, 0.0593])
# Under this a pixel's ratio is mostly the file's step, not the network.
FLOOR = 1.5e-4


def read(path):
    image = tifffile.imread(path)
    if image.dtype != np.uint16 or image.ndim != 3 or image.shape[2] < 3:
        sys.exit(f"{path}: want the CLI's 16-bit RGB TIFF, got {image.dtype} {image.shape}")
    return image[..., :3].astype(np.float64) / 65535.0


def compare(path_a, path_b):
    a, b = read(path_a), read(path_b)
    if a.shape != b.shape:
        sys.exit(f"{path_a} is {a.shape[1]}x{a.shape[0]}, {path_b} {b.shape[1]}x{b.shape[0]}")
    d = a - b
    psnr = 10 * np.log10(1 / max(np.mean(d * d), 1e-30))
    la, lb = a @ WEIGHTS, b @ WEIGHTS
    print(f"{path_a} against {path_b}: {a.shape[1]}x{a.shape[0]}, PSNR {psnr:.1f} dB")
    for name, lo, hi in BANDS:
        band = (la >= lo) & (la < hi)
        share = band.mean()
        ok = band & (la > FLOOR) & (lb > FLOOR)
        if not ok.any():
            print(f"  {name:12} {100 * share:5.1f}% of pixels")
            continue
        stops = np.log2(lb[ok] / la[ok])
        lift = []
        for c, channel in enumerate("RGB"):
            x, y = a[..., c][ok], b[..., c][ok]
            kept = (x > FLOOR) & (y > FLOOR)
            lift.append(f"{channel} {np.log2(y[kept] / x[kept]).mean():+.4f}")
        print(
            f"  {name:12} {100 * share:5.1f}% of pixels: |Δ| mean {np.abs(stops).mean():.4f} stops,"
            f" 99.9th {np.percentile(np.abs(stops), 99.9):.4f}; lift {', '.join(lift)}"
        )


def main():
    paths = sys.argv[1:]
    if not paths or len(paths) % 2:
        sys.exit(__doc__.split("\n\n")[1])
    for path_a, path_b in zip(paths[::2], paths[1::2]):
        compare(path_a, path_b)


if __name__ == "__main__":
    main()

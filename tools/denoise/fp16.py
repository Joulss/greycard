"""Convert the published fp32 denoisers to fp16, for Core ML.

    python fp16.py OUT_DIR MODEL.onnx [MODEL.onnx ...]

Each input must be a denoiser file the registry
(`crates/greycard-ai/src/registry.rs`) pins, byte for byte, so an fp16
file is always made from what greycard ships. The weights and the
arithmetic inside go to fp16; the input `packed` and the output `rgb`
stay float32 (`keep_io_types`), so the Rust side feeds and reads them
as it does the fp32 files, and the `greycard.*` metadata carries over.
Each file is written to OUT_DIR as `denoise-vNN-fp16.onnx`, and its
size and sha256 are printed for the registry.

Needs only `onnx` and `onnxconverter-common`, not torch. The published
files were made with onnxconverter-common 1.16.0 and onnx 1.23.0, and
measured on Core ML on an M5 (§481, §489). Another version may write
other bytes: check a new file on a Mac, through the CLI's
`--ai-denoise`, before publishing it.
"""

import argparse
import hashlib
import pathlib
import re
import sys
import warnings

import onnx
from onnxconverter_common import float16

REGISTRY = pathlib.Path(__file__).resolve().parents[2] / "crates/greycard-ai/src/registry.rs"
FLOAT = onnx.TensorProto.FLOAT


def pinned():
    """The registry's denoiser files: name to sha256."""
    text = REGISTRY.read_text()
    pairs = re.findall(
        r'name: "(denoise-v\d+\.onnx)",\s*url: "[^"]*",\s*bytes: [\d_]+,\s*sha256: "([0-9a-f]{64})"',
        text,
    )
    if not pairs:
        sys.exit(f"no denoiser files found in {REGISTRY}")
    return dict(pairs)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def convert(path, out_dir, pins):
    data = path.read_bytes()
    want = pins.get(path.name)
    if want is None:
        sys.exit(f"{path.name}: not a denoiser the registry pins ({', '.join(sorted(pins))})")
    if sha256(data) != want:
        sys.exit(f"{path}: sha256 {sha256(data)}, the registry pins {want}")

    model = onnx.load_from_string(data)
    # The converter warns for each weight it clamps to its floor of
    # ±1e-7, where fp16 would lose it; they are counted, not listed.
    with warnings.catch_warnings(record=True) as clamped:
        warnings.simplefilter("always")
        half = float16.convert_float_to_float16(model, keep_io_types=True)
    onnx.checker.check_model(half)

    # Checked with exits, not asserts, which `python -O` drops.
    meta = lambda m: {p.key: p.value for p in m.metadata_props}
    if meta(half) != meta(model):
        sys.exit(f"{path.name}: the metadata changed: {meta(model)} became {meta(half)}")
    names = lambda vs: [v.name for v in vs]
    if names(half.graph.input) != ["packed"] or names(half.graph.output) != ["rgb"]:
        sys.exit(f"{path.name}: want input packed and output rgb, got {names(half.graph.input)} and {names(half.graph.output)}")
    for v in list(half.graph.input) + list(half.graph.output):
        if v.type.tensor_type.elem_type != FLOAT:
            sys.exit(f"{path.name}: {v.name} is no longer float32")

    out = out_dir / path.name.replace(".onnx", "-fp16.onnx")
    blob = half.SerializeToString()
    out.write_bytes(blob)
    print(f"{out.name}: {len(blob):_} bytes, sha256 {sha256(blob)}, {len(clamped)} tiny weights clamped")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out_dir", type=pathlib.Path)
    p.add_argument("models", nargs="+", type=pathlib.Path)
    args = p.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)
    pins = pinned()
    for path in args.models:
        convert(path, args.out_dir, pins)


if __name__ == "__main__":
    main()

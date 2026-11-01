# §477. Subject and the denoisers on the Neural Engine, measured

*2026-10-31*

Two roadmap items waited on a Mac: Subject back on Core ML once its
Conv parses (§441), and the denoisers at fp16 so Core ML can put them
on the Neural Engine (§422). Both were tried on the M5 (macOS 27.0.1,
ONNX Runtime 1.28) with `probe`, which now builds its Core ML session
as the editor does (§466). Neither is taken up yet; this is what each
would cost.

## Subject: the pads parse, the compile does not finish

The Conv §441 named is not dilated. Twenty Convs in the rewrite, the
last of each deformable block (`aspp1`, `aspp_deforms.0` to `.2`,
kernels of 1, 3 and 7 at strides of the same), carry neither `pads`
nor `auto_pad`. ONNX Runtime's Core ML builder takes `NOTSET` with no
`pads` to `pad_type = valid` and adds no `pad` input
(`AddPadTypeAndPads`, `builder_utils.cc`, 1.28), which Core ML's parser
refuses; ONNX Runtime's own comment on the `same` case says the input
is wanted whatever the spec says (coremltools#2127). Writing
`pads = [0, 0, 0, 0]` on those twenty, ONNX's own default and so the
same graph, takes them through the `custom` path, and the parse
error goes.

What follows does not end. Core ML placed 3884 ops on the Neural
Engine, 174 on the GPU and 185 on the CPU, and Apple's
`ANECompilerService` then ran on one core for over 25 minutes, with
greycard's process asleep waiting on it, before the trial was stopped.
Kept off the Neural Engine (`coreml-gpu`), the session builds in 8.2 s
and the first run takes 15.8 s, and a run after that 3.6 s, against
WebGPU's 1.1 s session and 0.84 s a run. So Subject stays on WebGPU:
the GPU alone loses by four times, and the Neural Engine's compile
cannot be asked of a first mask. The rewrite with the pads is not
published.

## The denoisers at fp16

denoise-v20 (Best) converted with `onnxconverter-common`'s
`convert_float_to_float16`, fp32 in and out, at the editor's packed
tile of 608:

| denoise-v20 at 608            | session | a run   | ops                   |
|-------------------------------|---------|---------|-----------------------|
| fp32, every unit (today)      | 0.26 s  | 0.21 s  | 49 on the GPU         |
| fp32, CPU and Neural Engine   | 0.20 s  | 0.74 s  | 49 on the CPU         |
| fp16, CPU and Neural Engine   | 111.6 s | 0.12 s  | 48 Neural Engine, 9 CPU |
| fp16, every unit, compiled    | 0.82 s  | 0.08 s  | 47 Neural Engine, 10 GPU |

The last row followed the third, whose Neural Engine compile the
system had kept; cold, every unit would pay the 112 s too. A tile in
0.08 s against 0.21 s is 2.6 times faster: on a 102 MP frame, 108
tiles of 1024, about 14 s saved a denoise.

The answers move. On the probe's random input, fp32 Core ML is within
4.5e-6 of the CPU; fp16 on the Neural Engine is within 2.6e-3 of a
range of 0.28, 3.2e-4 on average. Whether that shows in a frame's
shadows, where a linear 2.6e-3 is a large share of the value, wants a
comparison on real raws before anything ships.

So fp16 waits on three things: that comparison; the first compile, near
two minutes on the M5 and kept by the system after, done before a
tester asks for a denoise (in the background at fetch or at launch);
and a published fp16 file for each tier, raced against fp32 by `open`
as Core ML and WebGPU are now.

## The other wait the roadmap named

The same day's 24 MP frame, edited with two generative fills, opened
in the editor with the fills' models loading on WebGPU: the 4-step
UNet in 27.1 s and the 25-step one in 40.3 s, each with the window
waiting. Those are the "first run under WebGPU sat for minutes" of
the roadmap's bug, or most of it; the bug now names them.

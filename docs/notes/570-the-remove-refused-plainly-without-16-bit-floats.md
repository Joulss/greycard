# §570. The remove refused plainly without 16-bit floats

*2026-11-05*

A tester's GTX 1080 fails every generative fill at the chain's first
model, with ONNX Runtime's shader compiler saying a Transpose needs
f16 and the device has none (issue 64). The remove's models are fp16
through and through, Fast and Best alike, and NVIDIA's cards before
Turing run no 16-bit floats in shaders. An fp32 export would not
help them: the UNet's weights are 5.1 GB in fp16 and would be 10 GB
in fp32, more than those cards hold. So it is a wall, and the only
thing wrong was how it was said.

The WebGPU probe that already names the adapter and reads its memory
now reads whether its shaders take 16-bit floats, and the engine
choice refuses the remove before any model is asked for, in words:
"the generative remove needs 16-bit floats in shaders, which NVIDIA
GeForce GTX 1080 has none of". The refusal is not a fill ahead, so
nothing is retried. Patch still fills on such a card. The README and
the guide say which cards.

# §563. ONNX Runtime's telemetry, off

*2026-11-04*

ONNX Runtime has no network code of its own. What it has is a
Windows ETW provider, Microsoft.ML.ONNXRuntime, tagged as Microsoft
telemetry so that Windows' diagnostic data can forward its events
under the user's privacy setting: the runtime's version, the model's
file name, graph and weight hashes and metadata, the providers and
devices, run counts and times, error messages, and the names of the
Windows services in the process. The provider is compiled in only
with `onnxruntime_USE_TELEMETRY`, off by default and on in Microsoft's
own release builds, where the events are enabled from the start.

Of the three libraries we load (§338), pyke's archive and the Intel
Mac build are source builds without it. The CPU-only fallback for a
processor without AVX2 is Microsoft's own 1.28.0 binary, with it; and
the `ort` crate's environment builder enables the events unless told
otherwise, which ours did not. So that one path, on Windows, logged
what the user ran to the OS. Nothing else did.

The environment now says `with_telemetry(false)`, a no-op on every
library but that one and insurance against a packager's swap. The
README says what the network is used for: a model or the lens
database fetched on first use, and the update check, which the
Settings sheet turns off.

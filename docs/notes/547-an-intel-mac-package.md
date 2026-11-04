# §547. An Intel Mac package

*2026-11-04*

A follow-on to §372: a fifth release leg, `x86_64-macos`, built natively
on GitHub's `macos-15-intel` runner (the x86-64 macOS 15 image, which
the runner-images README lists beside `macos-26-intel`; the macOS 14
images retired on 2026-11-02), signed and notarized as the Apple silicon
leg is, and rolled by the same `package.sh` into
`greycard-<version>-x86_64-macos.tar.gz` and `.dmg`. `intel-package.yml`
builds it by hand from any branch, as `arm-package.yml` does for Windows
on ARM.

## Why our own runtime

The plan was §372's: no runtime in the package, and Microsoft's own
fetched on first use. It cannot be. pyke's `dist.tsv` for ort-sys rc.13
lists `aarch64-apple-darwin` alone, and Microsoft stopped publishing
`osx-x86_64` (and `universal2`) archives after 1.23.2; PyPI's x86-64
macOS wheels stop at the same version. 1.23 is older than the API we
bind: `ort` asks `GetApi(27)`, which 1.23 cannot answer.

So we build 1.28.0 ourselves, once a version, in
`onnxruntime-intel-mac.yml`: Microsoft's `v1.28.0` tag, `build.sh
--config Release --build_shared_lib --use_coreml --parallel
--skip_tests`, natively on `macos-15-intel`, with the Xcode (26.3) and
CMake (3.31.8, Kitware's build, checked by hash) Microsoft's own macOS
CI pins. That native build is ours alone: Microsoft's x86-64 macOS build
of the tag is a cross compile, `--osx_arch x86_64` on the Apple silicon
`macos-15` with the tests skipped, and that is the fallback if the Intel
runner fails. No vcpkg and no Homebrew:
`FETCHCONTENT_TRY_FIND_PACKAGE_MODE=NEVER` takes every dependency from
`cmake/deps.txt`, and the workflow fails if the library links anything
outside `/usr/lib` and `/System/Library`, is not x86-64 alone, is not
for 13.4, or lacks `OrtGetApiBase` or Core ML. The archive goes on this
repository's pre-release `onnxruntime-1.28.0-x86_64-macos` (a
pre-release, so `releases/latest` stays on greycard's), and
`scripts/ort-intel-mac.sh` fetches it, checks its SHA-256, and puts the
library where `ort-dylib.sh` would; the leg's row says `runtime:
prebuilt`. The package ships it as the other legs ship theirs, so there
is no registry row and no first-use fetch: `SHIPS` is true on every Mac
and `cpu_runtime()` stays `None` there.

## No WebGPU, Core ML

We build it with Core ML, the Mac's own path, and without WebGPU, so no
Dawn ships beside it. A feature cannot default per target and every
dependent takes greycard-ai's defaults, so greycard-ai's build script
sets a `webgpu_ep` cfg (the feature, less x86-64 macOS) and the
runtime's WebGPU code keys on that. On an Intel Mac the providers are
Core ML, then the CPU; with no GPU provider after it, Core ML is taken
without the race. Core ML is only for the denoiser and the Sky prior
(Sky and Landscape); Subject, Object, People and LaMa leave it out
(`Provider::without_core_ml`) and run on the CPU, and the generative
remove, which runs on WebGPU alone, is refused as on Windows on ARM. The
processor check is skipped on a Mac: our build is plain x86-64 with its
kernels chosen at run time, and every Mac that runs 13.4 has x86-64-v3
anyway. Info.plist's floor stays 13.4, the build's deployment target.

## Untested

Nothing here has run. The runtime build has not been run (its SHA-256 in
`ort-intel-mac.sh` is empty until it is, and the leg fails plainly,
before its build, until then); whether 1.28 builds natively on Intel,
and for 13.4 rather than Microsoft's 14.0, is unknown. The leg and the
tester workflow have not run, and no Intel Mac has opened the package:
Core ML on an Intel GPU or AMD card, and the editor's own wgpu viewport
there, are untried. The leg is `tentative: true`, as §372's is.

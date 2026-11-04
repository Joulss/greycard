fn main() {
    // `webgpu_ep`: the WebGPU provider is offered, which is the
    // `webgpu` feature everywhere but an Intel Mac. There the package
    // ships our own build of ONNX Runtime with Core ML and without
    // WebGPU (pyke makes no x86-64 macOS build to take Dawn from), so
    // offering the provider would only fail a session build at every
    // model. A feature cannot default per target, and greycard-ai's
    // dependents take its defaults, so the target turns it off here.
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(webgpu_ep)");
    let feature = std::env::var_os("CARGO_FEATURE_WEBGPU").is_some();
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if feature && !(os == "macos" && arch == "x86_64") {
        println!("cargo::rustc-cfg=webgpu_ep");
    }
}

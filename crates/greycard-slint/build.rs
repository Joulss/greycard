fn main() {
    // A debug build, which is what the tests run, carries the element
    // names the testing backend finds controls by (a slider's value
    // field, the curve's In and Out); a release build goes without.
    let debug = std::env::var("PROFILE").as_deref() == Ok("debug");
    // The catalogs Slint bundles are found by the translation domain,
    // which slint-build takes from the package's name. The window's
    // strings and the Rust side's share one catalog per language,
    // `translations/<lang>/LC_MESSAGES/greycard.po`, so the domain is
    // "greycard": the name is set for this process alone, before any
    // thread of it exists. (A `with_translation_domain` setter on
    // slint-build would be the cleaner way.)
    // SAFETY: no other thread is running yet to read the environment.
    unsafe { std::env::set_var("CARGO_PKG_NAME", "greycard") };
    // The Slint compiler recurses over each expression, and the
    // window's key handler is deep enough to overflow the 1 MB a
    // build script's main thread has on Windows (Linux gives it 8).
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            // No default context: one English string is one entry,
            // wherever in the window it appears.
            let config = slint_build::CompilerConfiguration::new()
                .with_style("fluent-dark".into())
                .with_debug_info(debug)
                .with_bundled_translations("../../translations")
                .with_default_translation_context(slint_build::DefaultTranslationContext::None);
            slint_build::compile_with_config("ui/app.slint", config).expect("the Slint UI compiles")
        })
        .expect("a thread for the Slint compiler")
        .join()
        .expect("the Slint compiler's thread");
}

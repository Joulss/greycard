fn main() {
    // The domain set as greycard-slint sets it, so this proves the same
    // path: `translations/<lang>/LC_MESSAGES/greycard.po`, no default
    // context.
    // SAFETY: no other thread is running yet to read the environment.
    unsafe { std::env::set_var("CARGO_PKG_NAME", "greycard") };
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/probe.slint", config).expect("the probe compiles");
}

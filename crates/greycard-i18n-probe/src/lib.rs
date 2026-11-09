//! The window's translation mechanism, proved on a component of its
//! own: Slint's bundled catalogs under the domain "greycard" with no
//! default context, as greycard-slint builds them, selected at runtime
//! with `select_bundled_translation`. Its catalog is its own, so no
//! translator's edit can break the test; the window's catalogs are
//! the repository's alone.

slint::include_modules!();

#[cfg(test)]
mod tests {
    use super::*;

    /// French selected, the probe reads its translation, plurals by
    /// French's rule among them; English again with no language.
    #[test]
    fn a_bundled_catalog_is_selected_at_runtime() {
        i_slint_backend_testing::init_no_event_loop();
        let probe = Probe::new().expect("the probe builds");
        slint::select_bundled_translation("").expect("English is the source");
        assert_eq!(probe.get_word(), "Translation probe");
        assert_eq!(probe.get_frames(), "2 frames");
        slint::select_bundled_translation("fr").expect("French is bundled");
        assert_eq!(probe.get_word(), "Sonde de traduction");
        assert_eq!(probe.get_frames(), "2 vues");
        slint::select_bundled_translation("").expect("English is the source");
        assert_eq!(probe.get_word(), "Translation probe");
        assert!(slint::select_bundled_translation("de").is_err());
    }
}

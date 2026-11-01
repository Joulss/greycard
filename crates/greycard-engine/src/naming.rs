//! A level of a path made safe on every platform: what an export's
//! file name and the import's folders are cut to.

/// Characters Windows refuses in a name, beside the controls.
pub const REFUSED: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Names Windows keeps for devices, whatever follows the first dot.
const RESERVED: [&str; 6] = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"];

/// The longest a name is let be, in bytes: under every file system's
/// 255, with room for the import's temporary prefix and suffix.
pub const LONGEST: usize = 200;

/// One level of a path made safe on every platform: each refused
/// character and control is `_`; leading and trailing spaces and
/// trailing dots go (Windows drops them, so two names could meet);
/// a leading dot is `_` (it would hide the file on Linux and macOS,
/// and a name of dots alone is `.` or `..`); a reserved device name
/// takes `_` before it; and the name is cut to [`LONGEST`] bytes on a
/// character's edge. Empty when nothing is left.
pub fn safe(name: &str) -> String {
    safe_within(name, LONGEST)
}

/// [`safe`], cut to `limit` bytes rather than [`LONGEST`].
pub fn safe_within(name: &str, limit: usize) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_control() || REFUSED.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    if s.len() > limit {
        let mut cut = limit;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
    let trimmed = s.trim_start_matches(' ').trim_end_matches([' ', '.']);
    let mut s = trimmed.to_string();
    if s.starts_with('.') {
        s.replace_range(0..1, "_");
    }
    let device = s.split('.').next().unwrap_or_default().trim_end();
    let upper = device.to_ascii_uppercase();
    // COM and LPT with a digit, the superscript digits Windows counts
    // as one too (two bytes each, so matched by characters).
    let numbered = |prefix: &str| {
        let mut chars = upper.chars();
        upper.starts_with(prefix)
            && chars.by_ref().take(3).count() == 3
            && matches!(chars.next(), Some(c) if c.is_ascii_digit() || "¹²³".contains(c))
            && chars.next().is_none()
    };
    if RESERVED.contains(&upper.as_str()) || numbered("COM") || numbered("LPT") {
        s.insert(0, '_');
    }
    s
}

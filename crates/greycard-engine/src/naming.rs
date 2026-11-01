//! Naming patterns: `{date}/{camera}` for a folder, `{yyyy}{mm}{dd}-{seq}`
//! for a file, the tokens filled from one frame's fields.
//!
//! Pure: the fields come in, a name comes out, and nothing here reads
//! the disk or a clock. The import's subfolder and rename patterns use
//! it, and so does the export sheet's Name.
//!
//! Every name is made safe for the strictest file system it could land
//! on, whatever this machine is: a card imported on Linux onto an
//! exFAT drive meets Windows' rules, and a folder copied to a Windows
//! machine later meets them too. So Windows' refused characters, its
//! reserved device names and its trailing dots and spaces are dealt
//! with on every platform, and a name is the same on all three.
//!
//! Each level of a path, made safe on every platform, is what an
//! export's file name and the import's folders are cut to ([`safe`]).

/// A calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Day {
    /// The day of an EXIF date in either spelling, `2026:09:24 ...`
    /// as the file has it or `2026-09-24 ...` as the index keeps it;
    /// none when it is not one.
    pub fn from_exif(taken: &str) -> Option<Day> {
        let s: Vec<char> = taken.trim().chars().take(10).collect();
        if s.len() != 10 || !matches!(s[4], ':' | '-') || s[7] != s[4] {
            return None;
        }
        let num = |a: usize, b: usize| -> Option<u32> {
            s[a..b]
                .iter()
                .collect::<String>()
                .parse::<u32>()
                .ok()
                .filter(|_| s[a..b].iter().all(char::is_ascii_digit))
        };
        let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
        (year > 0 && (1..=12).contains(&month) && (1..=31).contains(&day)).then_some(Day {
            year: year as i32,
            month,
            day,
        })
    }
}

/// What the tokens of one frame are filled from.
#[derive(Debug, Clone, PartialEq)]
pub struct Fields {
    /// The day it was taken: the EXIF's, or the file's modification
    /// day when the file says none.
    pub day: Day,
    /// The source file's name without its extension.
    pub name: String,
    /// The camera as the panel names it; empty when unknown.
    pub camera: String,
    /// The frame's place in the run, from one.
    pub seq: usize,
    /// The frame's folder under the import's source, its levels
    /// joined by `/`; empty for a frame in the source itself.
    pub folder: String,
}

/// The tokens a pattern may hold, for the sheet's hint and the error.
pub const TOKENS: [&str; 8] = [
    "{date}", "{yyyy}", "{mm}", "{dd}", "{name}", "{camera}", "{seq}", "{folder}",
];

/// The tokens an export's Name may hold.
pub const EXPORT_TOKENS: [&str; 4] = ["{name}", "{seq}", "{date}", "{camera}"];

/// What an empty `{folder}` takes with it from beside it: the
/// characters a pattern puts between its parts.
const JOINERS: [char; 6] = ['-', '_', ' ', '.', '/', '\\'];

/// `{seq}`'s width: 0001.
pub const SEQ_WIDTH: usize = 4;

/// Why a pattern makes no name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternError {
    /// A `{...}` that is not one of [`TOKENS`].
    Unknown(String),
    /// A `{` with no `}` after it.
    Unclosed,
    /// An export's Name that fills to nothing a file can be named.
    Empty,
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(t) => write!(f, "{{{t}}} is not a token; {}", TOKENS.join(" ")),
            Self::Unclosed => write!(f, "a {{ with no }} after it"),
            Self::Empty => write!(f, "it names nothing"),
        }
    }
}

impl std::error::Error for PatternError {}

/// The pattern with its tokens filled, the literal text between them
/// as it is. A token's value never adds a folder: a separator in it
/// is refused as any other character is. `{folder}` alone keeps its
/// levels, `/` between them, so in the Folders pattern it makes the
/// source's folders again; in a name [`file_name`] turns its `/`
/// into `_` as it does the pattern's own. Nothing else is made safe
/// here; [`file_name`] and [`folders`] do that.
///
/// `{seq:N}` counts from `N` rather than one, at least as wide as `N`
/// is typed, and `{date:FORMAT}` writes the day by `%Y`, `%y`, `%m`,
/// and `%d` (`%%` for a `%`) among literal text.
pub fn expand(pattern: &str, f: &Fields) -> Result<String, PatternError> {
    fill(pattern, f, &TOKENS, None)
}

/// An export's file name, without its extension, from the sheet's
/// Name pattern: only [`EXPORT_TOKENS`], `{seq}` as wide as the last
/// number of the `count` frames exported needs, and made safe as an
/// import's name is. An unknown or unclosed token is an error, and so
/// is a pattern that fills to nothing.
pub fn export_stem(pattern: &str, f: &Fields, count: usize) -> Result<String, PatternError> {
    let stem = fill(pattern, f, &EXPORT_TOKENS, Some(count))?.replace(['/', '\\'], "_");
    let stem = safe(&stem);
    if stem.is_empty() {
        return Err(PatternError::Empty);
    }
    Ok(stem)
}

/// [`expand`] over `tokens` alone, `{seq}` as wide as [`SEQ_WIDTH`]
/// or, given the run's `count`, as its last number needs.
fn fill(
    pattern: &str,
    f: &Fields,
    tokens: &[&str],
    count: Option<usize>,
) -> Result<String, PatternError> {
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut rest = pattern;
    // Whether `out` ends in the pattern's own text rather than a
    // token's value: only a separator the pattern wrote is one an empty
    // `{folder}` may take with it.
    let mut ends_literal = false;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        if open > 0 {
            ends_literal = true;
        }
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or(PatternError::Unclosed)?;
        let token = &after[..close];
        let (word, argument) = match token.split_once(':') {
            Some((word, argument)) => (word, Some(argument)),
            None => (token, None),
        };
        if !tokens.contains(&format!("{{{word}}}").as_str()) {
            return Err(PatternError::Unknown(token.to_string()));
        }
        let unknown = || PatternError::Unknown(token.to_string());
        let value = match (word, argument) {
            ("seq", argument) => {
                let start = match argument {
                    None => 1,
                    Some(a) if !a.is_empty() && a.bytes().all(|b| b.is_ascii_digit()) => {
                        a.parse::<usize>().map_err(|_| unknown())?
                    }
                    Some(_) => return Err(unknown()),
                };
                let at = start.saturating_add(f.seq.saturating_sub(1));
                let width = match count {
                    Some(n) => digits(start.saturating_add(n.max(1) - 1)),
                    None => SEQ_WIDTH,
                }
                .max(argument.map_or(0, str::len));
                format!("{at:0width$}")
            }
            ("date", Some(format)) => day_as(f.day, format).ok_or_else(unknown)?,
            (_, Some(_)) => return Err(unknown()),
            (word, None) => match word {
                "date" => format!("{:04}-{:02}-{:02}", f.day.year, f.day.month, f.day.day),
                "yyyy" => format!("{:04}", f.day.year),
                "mm" => format!("{:02}", f.day.month),
                "dd" => format!("{:02}", f.day.day),
                "name" => f.name.clone(),
                "camera" if f.camera.trim().is_empty() => "Unknown camera".into(),
                "camera" => f.camera.trim().to_string(),
                // Its levels are `/` alone (`import::folder_under` joins
                // them so on every platform); a `\` is a character of a
                // folder's name on Linux, and refused as any other is.
                // Empty, it takes a separator beside it with it, so
                // `{folder}-{name}` in the source itself is `{name}`.
                "folder" if f.folder.is_empty() => {
                    rest = &after[close + 1..];
                    match rest.chars().next() {
                        Some(c) if JOINERS.contains(&c) => rest = &rest[c.len_utf8()..],
                        _ => {
                            if ends_literal && out.ends_with(JOINERS) {
                                out.pop();
                            }
                        }
                    }
                    continue;
                }
                "folder" => {
                    out.push_str(&f.folder.replace('\\', "_"));
                    ends_literal = false;
                    rest = &after[close + 1..];
                    continue;
                }
                other => return Err(PatternError::Unknown(other.to_string())),
            },
        };
        out.push_str(&value.replace(['/', '\\'], "_"));
        if !value.is_empty() {
            ends_literal = false;
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// How many digits `n` is written in.
fn digits(n: usize) -> usize {
    n.checked_ilog10().map_or(1, |l| l as usize + 1)
}

/// `day` by a `{date:...}` format: `%Y`, `%y`, `%m`, `%d`, and `%%`,
/// the rest as it is. None for any other `%`.
fn day_as(day: Day, format: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'Y' => out.push_str(&format!("{:04}", day.year)),
            'y' => out.push_str(&format!("{:02}", day.year.rem_euclid(100))),
            'm' => out.push_str(&format!("{:02}", day.month)),
            'd' => out.push_str(&format!("{:02}", day.day)),
            '%' => out.push('%'),
            _ => return None,
        }
    }
    Some(out)
}

/// A file's name from the rename pattern, with `extension` (as the
/// source spells it) after it; the source's own stem when the pattern
/// fills to nothing.
pub fn file_name(pattern: &str, f: &Fields, extension: &str) -> Result<String, PatternError> {
    file_name_within(pattern, f, extension, 0)
}

/// [`file_name`] with `reserve` bytes more kept free after the stem: a
/// frame whose companion's tail (`.CR3.xmp`) is longer than its own
/// extension leaves room for it, so the two keep one stem however long
/// the pattern makes it.
pub fn file_name_within(
    pattern: &str,
    f: &Fields,
    extension: &str,
    reserve: usize,
) -> Result<String, PatternError> {
    let longest = LONGEST.saturating_sub(reserve);
    let stem = expand(pattern, f)?.replace(['/', '\\'], "_");
    let tail = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    let mut name = with_tail_in(&stem, &tail, longest);
    if name.len() == tail.len() {
        name = with_tail_in(&f.name, &tail, longest);
    }
    if name.len() == tail.len() {
        name = format!("_{tail}");
    }
    Ok(name)
}

/// `stem` made safe and cut so that `tail` (an extension with its dot,
/// or a companion's `.CR3.xmp`) always fits after it within
/// [`LONGEST`]: a long pattern loses the end of its stem, never its
/// extension, and never half a character.
pub fn with_tail(stem: &str, tail: &str) -> String {
    with_tail_in(stem, tail, LONGEST)
}

/// [`with_tail`] within `longest` bytes rather than [`LONGEST`].
pub fn with_tail_in(stem: &str, tail: &str, longest: usize) -> String {
    let tail: String = tail
        .chars()
        .map(|c| {
            if c.is_control() || REFUSED.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let room = longest.saturating_sub(tail.len()).max(1);
    format!("{}{tail}", safe_within(stem, room))
}

/// The folders under the destination from the subfolder pattern, one
/// entry a level: `/` and `\` in the pattern's literal text separate
/// them, an empty level is dropped, and each is made safe. Empty for
/// an empty pattern: the files go in the destination itself.
pub fn folders(pattern: &str, f: &Fields) -> Result<Vec<String>, PatternError> {
    let expanded = expand(pattern, f)?;
    Ok(expanded
        .split(['/', '\\'])
        .map(safe)
        .filter(|c| !c.is_empty())
        .collect())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Fields {
        Fields {
            day: Day {
                year: 2026,
                month: 9,
                day: 4,
            },
            name: "IMG_0042".into(),
            camera: "Canon EOS R5".into(),
            seq: 7,
            folder: String::new(),
        }
    }

    #[test]
    fn every_token_fills() {
        let f = fields();
        assert_eq!(expand("{date}", &f).unwrap(), "2026-09-04");
        assert_eq!(expand("{yyyy}", &f).unwrap(), "2026");
        assert_eq!(expand("{mm}", &f).unwrap(), "09");
        assert_eq!(expand("{dd}", &f).unwrap(), "04");
        assert_eq!(expand("{name}", &f).unwrap(), "IMG_0042");
        assert_eq!(expand("{camera}", &f).unwrap(), "Canon EOS R5");
        assert_eq!(expand("{seq}", &f).unwrap(), "0007");
        // Literal text between them, as it is.
        assert_eq!(
            expand("{yyyy}{mm}{dd}_{camera} - {name}-{seq}!", &f).unwrap(),
            "20260904_Canon EOS R5 - IMG_0042-0007!"
        );
        // The width is a floor, not a cap.
        let big = Fields {
            seq: 123456,
            ..fields()
        };
        assert_eq!(expand("{seq}", &big).unwrap(), "123456");
        // No camera said: a word, not a hole in the name.
        let none = Fields {
            camera: " ".into(),
            ..fields()
        };
        assert_eq!(expand("{camera}", &none).unwrap(), "Unknown camera");
    }

    #[test]
    fn an_export_name_fills_its_four_tokens_among_literal_text() {
        let f = fields();
        let stem = |p: &str, count: usize| export_stem(p, &f, count);
        assert_eq!(stem("{name}", 1).unwrap(), "IMG_0042");
        assert_eq!(stem("{date}", 1).unwrap(), "2026-09-04");
        assert_eq!(stem("{date:%Y%m%d}", 1).unwrap(), "20260904");
        assert_eq!(stem("{date:%y.%m.%d %%}", 1).unwrap(), "26.09.04 %");
        assert_eq!(stem("{camera}", 1).unwrap(), "Canon EOS R5");
        assert_eq!(
            stem("Trip {date} - {camera} ({name})", 1).unwrap(),
            "Trip 2026-09-04 - Canon EOS R5 (IMG_0042)"
        );
        // `{seq}` is as wide as the set's last number needs: the 7th of
        // 9, of 12 and of 100.
        assert_eq!(stem("{seq}", 9).unwrap(), "7");
        assert_eq!(stem("{seq}", 12).unwrap(), "07");
        assert_eq!(stem("{seq}", 100).unwrap(), "007");
        // From a start, as wide as the last or as typed.
        assert_eq!(stem("{seq:101}", 5).unwrap(), "107");
        assert_eq!(stem("{seq:95}", 10).unwrap(), "101");
        assert_eq!(stem("{seq:0001}", 9).unwrap(), "0007");
        let first = Fields { seq: 1, ..fields() };
        assert_eq!(export_stem("{seq:95}", &first, 10).unwrap(), "095");
        // Made safe as an import's name is.
        assert_eq!(stem("{name}: a/b", 1).unwrap(), "IMG_0042_ a_b");
    }

    #[test]
    fn an_export_name_refuses_what_it_does_not_know_and_nothing() {
        let f = fields();
        for bad in [
            "{yyyy}",
            "{folder}",
            "{lens}",
            "{name:x}",
            "{camera:short}",
            "{seq:}",
            "{seq:abc}",
            "{seq:-1}",
            "{date:%H}",
            "{date:%}",
        ] {
            let word = bad.trim_matches(['{', '}']).to_string();
            assert_eq!(
                export_stem(bad, &f, 1),
                Err(PatternError::Unknown(word)),
                "{bad}"
            );
        }
        assert_eq!(export_stem("{name", &f, 1), Err(PatternError::Unclosed));
        for empty in ["", "  ", "..."] {
            assert_eq!(export_stem(empty, &f, 1), Err(PatternError::Empty));
        }
        // The import keeps its own tokens and its fixed width, and
        // takes a start and a date's format too.
        assert_eq!(expand("{yyyy}-{seq:101}", &f).unwrap(), "2026-0107");
        assert_eq!(expand("{date:%d.%m.%Y}", &f).unwrap(), "04.09.2026");
    }

    /// `{folder}` makes the source's folders again under the Folders
    /// pattern, a level a level, and nothing for a frame in the source
    /// itself; in a name its levels are joined by `_`.
    #[test]
    fn the_folder_token_keeps_its_levels_in_folders_alone() {
        let f = Fields {
            folder: "100CANON/day two".into(),
            ..fields()
        };
        assert_eq!(expand("{folder}", &f).unwrap(), "100CANON/day two");
        assert_eq!(folders("{folder}", &f).unwrap(), ["100CANON", "day two"]);
        assert_eq!(
            folders("{date}/{folder}", &f).unwrap(),
            ["2026-09-04", "100CANON", "day two"]
        );
        assert_eq!(
            file_name("{folder}-{name}", &f, "CR3").unwrap(),
            "100CANON_day two-IMG_0042.CR3"
        );
        let top = fields();
        assert!(folders("{folder}", &top).unwrap().is_empty());
        assert_eq!(folders("{date}/{folder}", &top).unwrap(), ["2026-09-04"]);
        // Empty, it takes the separator beside it, after or before.
        assert_eq!(
            file_name("{folder}-{name}", &top, "CR3").unwrap(),
            "IMG_0042.CR3"
        );
        assert_eq!(
            file_name("{name}_{folder}", &top, "CR3").unwrap(),
            "IMG_0042.CR3"
        );
        assert_eq!(folders("{folder}/{date}", &top).unwrap(), ["2026-09-04"]);
        // Never a character of a token's value: the name's own
        // trailing `_` stays.
        let trailing = Fields {
            name: "IMG_0001_".into(),
            ..fields()
        };
        assert_eq!(
            file_name("{name}{folder}", &trailing, "CR3").unwrap(),
            "IMG_0001_.CR3"
        );
        assert_eq!(
            file_name("{name}-{folder}", &trailing, "CR3").unwrap(),
            "IMG_0001_.CR3"
        );
        // A `\` in a Linux folder's name is a character, not a level.
        let slashed = Fields {
            folder: "back\\slash/day".into(),
            ..fields()
        };
        assert_eq!(
            folders("{folder}", &slashed).unwrap(),
            ["back_slash", "day"]
        );
        // Each level made safe as any other.
        let odd = Fields {
            folder: "a:b/ c. ".into(),
            ..fields()
        };
        assert_eq!(folders("{folder}", &odd).unwrap(), ["a_b", "c"]);
        // And none climbs out: a level of dots is no level.
        let up = Fields {
            folder: "../x/./y".into(),
            ..fields()
        };
        assert_eq!(folders("{folder}", &up).unwrap(), ["x", "y"]);
    }

    #[test]
    fn a_pattern_with_no_token_is_its_own_text() {
        let f = fields();
        assert_eq!(expand("holiday", &f).unwrap(), "holiday");
        assert_eq!(expand("", &f).unwrap(), "");
        // The same name for every frame: the import tells them apart.
        assert_eq!(file_name("holiday", &f, "CR3").unwrap(), "holiday.CR3");
        // Nothing at all falls back to the source's own name.
        assert_eq!(file_name("", &f, "CR3").unwrap(), "IMG_0042.CR3");
        assert_eq!(folders("", &f).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn an_unknown_or_unclosed_token_is_refused() {
        let f = fields();
        assert_eq!(
            expand("{name}-{lens}", &f),
            Err(PatternError::Unknown("lens".into()))
        );
        assert_eq!(expand("{name", &f), Err(PatternError::Unclosed));
        // A token is spelled in lower case, as the hint shows it.
        assert!(expand("{DATE}", &f).is_err());
        assert!(
            PatternError::Unknown("lens".into())
                .to_string()
                .contains("{seq}")
        );
    }

    /// Two frames of one name from two folders of the card (the
    /// counter wrapped at 9999, or two bodies): the same pattern
    /// meets itself, and `{seq}` parts them.
    #[test]
    fn a_collision_is_made_unique_by_seq() {
        let a = Fields { seq: 1, ..fields() };
        let b = Fields { seq: 2, ..fields() };
        assert_eq!(
            file_name("{name}", &a, "CR3").unwrap(),
            file_name("{name}", &b, "CR3").unwrap()
        );
        let (na, nb) = (
            file_name("{name}-{seq}", &a, "CR3").unwrap(),
            file_name("{name}-{seq}", &b, "CR3").unwrap(),
        );
        assert_ne!(na, nb);
        assert_eq!(na, "IMG_0042-0001.CR3");
        assert_eq!(nb, "IMG_0042-0002.CR3");
    }

    #[test]
    fn a_name_the_file_system_refuses_is_made_safe() {
        let f = Fields {
            name: "a<b>c:d\"e|f?g*h\u{7}".into(),
            camera: "Maker/Model\\II".into(),
            ..fields()
        };
        // Every refused character and the control: `_`.
        assert_eq!(
            file_name("{name}", &f, "CR3").unwrap(),
            "a_b_c_d_e_f_g_h_.CR3"
        );
        // A token's separator never makes a folder.
        assert_eq!(
            folders("{camera}/{date}", &f).unwrap(),
            vec!["Maker_Model_II".to_string(), "2026-09-04".to_string()]
        );
        assert_eq!(
            file_name("{camera}", &f, "jpg").unwrap(),
            "Maker_Model_II.jpg"
        );
        // Windows' devices, in any case and before any dot.
        for device in [
            "CON",
            "prn",
            "Aux",
            "nul",
            "COM1",
            "lpt9",
            "con.txt",
            "COM¹",
            "com²",
            "COM³",
            "LPT¹",
            "lpt²",
            "LPT³.txt",
            "CONIN$",
            "conout$",
            "CONOUT$.log",
        ] {
            let named = Fields {
                name: device.into(),
                ..fields()
            };
            let got = file_name("{name}", &named, "CR3").unwrap();
            assert!(got.starts_with('_'), "{device}: {got}");
        }
        // Not a device: a longer word, a COM with no digit.
        assert_eq!(safe("CONSOLE"), "CONSOLE");
        assert_eq!(safe("COMX"), "COMX");
        assert_eq!(safe("COM¹²"), "COM¹²");
        assert_eq!(safe("CONIN"), "CONIN");
        // Trailing dots and spaces go, a leading dot is not a hidden
        // file, and dots alone are not `.` or `..`.
        assert_eq!(safe("  shoot. . "), "shoot");
        assert_eq!(safe(".hidden"), "_hidden");
        assert_eq!(safe(".."), "");
        assert_eq!(
            folders("{yyyy}/../{mm}//./", &fields()).unwrap(),
            vec!["2026".to_string(), "09".to_string()]
        );
        // A name ending in dots before the extension keeps no dot run
        // a Windows file system would drop: the stem is trimmed first.
        let dotted = Fields {
            name: "last.".into(),
            ..fields()
        };
        assert_eq!(file_name("{name}", &dotted, "CR3").unwrap(), "last.CR3");
        // Long names are cut on a character's edge.
        let long = "é".repeat(300);
        let cut = safe(&long);
        assert!(cut.len() <= LONGEST && cut.chars().all(|c| c == 'é'));
    }

    /// A 197-letter pattern with `{seq}` is cut in its stem: the
    /// extension is kept whole, the name within the limit, and never
    /// cut inside a character.
    #[test]
    fn a_long_name_keeps_its_extension() {
        let pattern = format!("{}{{seq}}", "a".repeat(197));
        for seq in [1, 2] {
            let f = Fields { seq, ..fields() };
            let name = file_name(&pattern, &f, "CR3").unwrap();
            assert!(name.ends_with(".CR3"), "{name}");
            assert!(name.len() <= LONGEST, "{}", name.len());
        }
        // Where the stem's room allows, the digits survive.
        let f = Fields { seq: 7, ..fields() };
        let name = file_name(&format!("{}{{seq}}", "a".repeat(190)), &f, "CR3").unwrap();
        assert!(name.ends_with("0007.CR3"), "{name}");
        // A multibyte stem is cut on a character's edge.
        let wide = Fields {
            name: "é".repeat(150),
            ..fields()
        };
        let name = file_name("{name}", &wide, "jpeg").unwrap();
        assert!(name.ends_with(".jpeg") && name.len() <= LONGEST, "{name}");
        // A companion's longer tail is kept whole too.
        let c = with_tail(&"b".repeat(250), ".CR3.xmp");
        assert!(c.ends_with(".CR3.xmp") && c.len() <= LONGEST);
    }

    #[test]
    fn a_day_reads_from_either_spelling() {
        let want = Some(Day {
            year: 2024,
            month: 8,
            day: 24,
        });
        assert_eq!(Day::from_exif("2024:08:24 15:32:39"), want);
        assert_eq!(Day::from_exif("2024-08-24 15:32:39"), want);
        assert_eq!(Day::from_exif("2024-08-24"), want);
        assert_eq!(Day::from_exif("    :  :     "), None);
        assert_eq!(Day::from_exif("2024:13:01"), None);
        assert_eq!(Day::from_exif("2024:08-24"), None);
        assert_eq!(Day::from_exif(""), None);
    }
}

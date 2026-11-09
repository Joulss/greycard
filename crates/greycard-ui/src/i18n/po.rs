//! A reader of gettext's `.po` catalogs: the subset `msgmerge` and a
//! translator's editor write. Comments are skipped, an entry marked
//! fuzzy is kept with its mark, and an obsolete one (`#~`) is dropped.
//! Used by greycard-ui's build script, which turns each catalog into a
//! static table, and by the tests, which read catalogs of their own.

/// One message of a catalog.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Entry {
    pub context: Option<String>,
    pub id: String,
    pub plural_id: Option<String>,
    /// `msgstr`, or `msgstr[0]`, `msgstr[1]`, ... for a plural.
    pub strs: Vec<String>,
    pub fuzzy: bool,
}

impl Entry {
    /// The key a message is found by: its context and its English, as
    /// gettext joins them, or the English alone.
    pub fn key(&self) -> String {
        match &self.context {
            Some(c) => format!("{c}\u{4}{}", self.id),
            None => self.id.clone(),
        }
    }

    /// Whether the entry is a translation to use: not fuzzy, and no
    /// form of it left empty.
    pub fn translated(&self) -> bool {
        !self.fuzzy
            && !self.id.is_empty()
            && !self.strs.is_empty()
            && self.strs.iter().all(|s| !s.is_empty())
    }
}

/// A catalog: the header's fields and the messages.
#[derive(Debug, Default)]
pub struct Po {
    pub header: Vec<(String, String)>,
    pub entries: Vec<Entry>,
}

impl Po {
    /// The header field `name`, trimmed.
    pub fn field(&self, name: &str) -> Option<&str> {
        self.header
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Which string a continuation line adds to.
#[derive(Clone, Copy)]
enum Field {
    None,
    Context,
    Id,
    PluralId,
    Str(usize),
}

/// Parse a catalog's text; an error names the line.
pub fn parse(text: &str) -> Result<Po, String> {
    let mut entries = Vec::new();
    let mut current = Entry::default();
    let mut started = false;
    let mut fuzzy_next = false;
    let mut field = Field::None;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (number, line) in text.lines().enumerate() {
        let number = number + 1;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(flags) = line.strip_prefix("#,") {
            if flags.split(',').any(|f| f.trim() == "fuzzy") {
                fuzzy_next = true;
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with('"') {
            let s = unquote(line).map_err(|e| format!("line {number}: {e}"))?;
            match field {
                Field::None => return Err(format!("line {number}: a string outside an entry")),
                Field::Context => current.context.get_or_insert_with(String::new).push_str(&s),
                Field::Id => current.id.push_str(&s),
                Field::PluralId => current
                    .plural_id
                    .get_or_insert_with(String::new)
                    .push_str(&s),
                Field::Str(i) => current.strs[i].push_str(&s),
            }
            continue;
        }
        let (keyword, rest) = line
            .split_once(|c: char| c.is_whitespace())
            .ok_or_else(|| format!("line {number}: no string after {line:?}"))?;
        let s = unquote(rest.trim()).map_err(|e| format!("line {number}: {e}"))?;
        // A new entry begins at its context, or at its English when
        // the one before has a translation.
        let begins =
            keyword == "msgctxt" || (keyword == "msgid" && !matches!(field, Field::Context));
        if begins {
            if started {
                entries.push(std::mem::take(&mut current));
            }
            started = true;
            current.fuzzy = std::mem::take(&mut fuzzy_next);
        }
        field = match keyword {
            "msgctxt" => {
                current.context = Some(s);
                Field::Context
            }
            "msgid" => {
                current.id = s;
                Field::Id
            }
            "msgid_plural" => {
                current.plural_id = Some(s);
                Field::PluralId
            }
            "msgstr" => {
                current.strs = vec![s];
                Field::Str(0)
            }
            k => {
                let index = k
                    .strip_prefix("msgstr[")
                    .and_then(|r| r.strip_suffix(']'))
                    .and_then(|i| i.parse::<usize>().ok())
                    .ok_or_else(|| format!("line {number}: unknown keyword {k:?}"))?;
                if current.strs.len() <= index {
                    current.strs.resize(index + 1, String::new());
                }
                current.strs[index] = s;
                Field::Str(index)
            }
        };
    }
    if started {
        entries.push(current);
    }
    let mut header = Vec::new();
    if let Some(at) = entries
        .iter()
        .position(|e| e.id.is_empty() && e.context.is_none())
    {
        let entry = entries.remove(at);
        for line in entry.strs.first().map(String::as_str).unwrap_or("").lines() {
            if let Some((k, v)) = line.split_once(':') {
                header.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    Ok(Po { header, entries })
}

/// A quoted string's contents, its escapes undone: C's, octal and hex
/// among them.
fn unquote(s: &str) -> Result<String, String> {
    let inner = s
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .filter(|_| s.len() >= 2)
        .ok_or_else(|| format!("{s:?} is not a quoted string"))?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('?') => out.push('?'),
            Some('a') => out.push('\u{7}'),
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('v') => out.push('\u{b}'),
            Some(d @ '0'..='7') => {
                let mut value = d.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    match chars.peek().and_then(|c| c.to_digit(8)) {
                        Some(v) => {
                            value = value * 8 + v;
                            chars.next();
                        }
                        None => break,
                    }
                }
                out.push(char::from_u32(value).ok_or_else(|| format!("\\{value:o} in {s:?}"))?);
            }
            Some('x') => {
                let mut value = 0u32;
                let mut digits = 0;
                while let Some(v) = chars.peek().and_then(|c| c.to_digit(16)) {
                    value = value.saturating_mul(16).saturating_add(v);
                    digits += 1;
                    chars.next();
                }
                if digits == 0 {
                    return Err(format!("\\x with no digits in {s:?}"));
                }
                out.push(char::from_u32(value).ok_or_else(|| format!("\\x{value:x} in {s:?}"))?);
            }
            Some(other) => return Err(format!("unknown escape \\{other} in {s:?}")),
            None => return Err(format!("{s:?} ends in a backslash")),
        }
    }
    Ok(out)
}

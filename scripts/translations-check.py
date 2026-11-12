#!/usr/bin/env python3
# Whether the translators' catalogs match the window's source, without
# building anything and without gettext's tools:
#
#     scripts/translations-check.py          errors, then counts
#     scripts/translations-check.py --list   every reported string too
#     scripts/translations-check.py --self-test   its readers on small cases
#
# 1. The template against the source. Every string the source marks
#    (`@tr(...)` in the Slint files, `tr!` and `marked::tr!` in
#    greycard-ui) has its entry in translations/greycard.pot, and every
#    entry there is still marked somewhere. A difference is an error: an
#    entry to add by hand, at its file's place, or one to take out.
# 2. Each language against the template. Strings it lacks, leaves
#    empty, marks fuzzy, or holds that the template no longer has, as
#    a report: a catalog behind the source is normal. A translation
#    whose placeholders (`{}`, `{0}`, `{name}`) differ from its
#    English's is an error: the value is lost or the braces shown.
# 3. Words the window may show that nothing marks: a string with
#    letters in the Slint files bound outside `@tr` to a property the
#    scan in crates/greycard-slint/tests/untranslated.rs does not read
#    (that test fails on the ones it reads), and one in greycard-ui's
#    Rust handed to a `set_` call or a `SharedString` outside `tr!`.
#    Candidates, as a report; translations/untranslated.txt lists the
#    ones that are data or keys.
#
# Exits 1 on an error, 0 otherwise. Python 3.9 or later, no packages.
import argparse
import os
import re
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SLINT_DIR = "crates/greycard-slint/ui"
RUST_DIR = "crates/greycard-ui/src"
POT = "translations/greycard.pot"
ALLOW = "translations/untranslated.txt"
SLINT_ALLOW = "crates/greycard-slint/ui/untranslated.txt"

# The bindings tests/untranslated.rs reads, which it fails on.
SCANNED = {
    "text", "words", "placeholder-text", "placeholder", "accessible-label",
    "accessible-description", "accessible-value", "title", "label", "note",
    "hint", "tooltip", "tips", "empty", "options", "labels",
}
# Rust macros whose strings are for the log, a developer, or a test.
NOT_SHOWN_MACROS = {
    "trace", "debug", "info", "warn", "error", "event", "span", "panic",
    "assert", "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq",
    "debug_assert_ne", "unreachable", "todo", "unimplemented", "println",
    "eprintln", "print", "eprint", "write", "writeln", "include_str",
    "include_bytes", "env", "concat", "matches", "anyhow", "bail", "ensure",
    "stringify", "cfg", "shown",
}
# Rust calls whose strings are for a developer.
NOT_SHOWN_CALLS = {"expect", "context", "with_context", "unwrap_or_else"}

WORD = re.compile(r"[A-Za-z]{2}")


# ---------------------------------------------------------------- lexing


class Tok:
    __slots__ = ("kind", "text", "value", "line")

    def __init__(self, kind, text, value, line):
        self.kind = kind  # "str", "ident", "punct"
        self.text = text
        self.value = value  # a string's value
        self.line = line

    def __repr__(self):
        return f"{self.kind}:{self.text!r}@{self.line}"


PUNCT2 = ("=>", "::", "==", "!=", ":=", "<=>")


def rust_unescape(s):
    out = []
    i = 0
    while i < len(s):
        c = s[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        n = s[i + 1]
        if n == "\n" or n == "\r":
            i += 2
            while i < len(s) and s[i] in " \t\n\r":
                i += 1
            continue
        if n == "x":
            out.append(chr(int(s[i + 2:i + 4], 16)))
            i += 4
            continue
        if n == "u":
            close = s.index("}", i)
            out.append(chr(int(s[i + 3:close], 16)))
            i = close + 1
            continue
        out.append({"n": "\n", "r": "\r", "t": "\t", "0": "\0"}.get(n, n))
        i += 2
    return "".join(out)


def slint_unescape(s):
    out = []
    i = 0
    while i < len(s):
        c = s[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        n = s[i + 1]
        if n == "u":
            close = s.index("}", i)
            out.append(chr(int(s[i + 3:close], 16)))
            i = close + 1
            continue
        out.append({"n": "\n", "r": "\r", "t": "\t"}.get(n, n))
        i += 2
    return "".join(out)


def lex_rust(src):
    toks = []
    i, line, n = 0, 1, len(src)
    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            i += 1
        elif c in " \t\r":
            i += 1
        elif src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            line += src.count("\n", i, j)
            i = j
        elif c == '"' or (c in "bc" and src.startswith('"', i + 1)):
            start = i
            j = i + (1 if c == '"' else 2)
            while src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            raw = src[start + (1 if c == '"' else 2):j]
            toks.append(Tok("str" if c == '"' else "bytes", src[start:j + 1],
                            rust_unescape(raw), line))
            line += raw.count("\n")
            i = j + 1
        elif re.match(r'(?:br|cr|r)#*"', src[i:i + 40]):
            m = re.match(r'(b|c)?r(#*)"', src[i:i + 256])
            hashes = m.group(2)
            body = i + m.end()
            end = src.index('"' + hashes, body)
            raw = src[body:end]
            toks.append(Tok("str" if not m.group(1) else "bytes",
                            src[i:end + 1 + len(hashes)], raw, line))
            line += raw.count("\n")
            i = end + 1 + len(hashes)
        elif c == "'":
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\x[0-9a-fA-F]{2}|\\.|[^\\'])'", src[i:i + 256])
            if m:
                toks.append(Tok("char", m.group(0), None, line))
                i += m.end()
            else:  # a lifetime or a label
                m = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", src[i:i + 256])
                toks.append(Tok("ident", m.group(0), None, line))
                i += m.end()
        elif c.isalpha() or c == "_" or ord(c) > 127:
            m = re.match(r"[\w]+", src[i:i + 256])
            toks.append(Tok("ident", m.group(0), None, line))
            i += m.end()
        elif c.isdigit():
            m = re.match(r"[0-9][0-9A-Za-z_.]*", src[i:i + 256])
            text = m.group(0)
            if ".." in text:
                text = text[:text.index("..")]
            toks.append(Tok("num", text, None, line))
            i += len(text)
        else:
            p = next((p for p in PUNCT2 if src.startswith(p, i)), c)
            toks.append(Tok("punct", p, None, line))
            i += len(p)
    return toks


def slint_string_end(src, i):
    """One past the closing quote of the string opening at `i`; an
    interpolation `\\{...}` may hold strings of its own."""
    j = i + 1
    while j < len(src):
        c = src[j]
        if c == "\\" and src.startswith("{", j + 1):
            depth, j = 0, j + 1
            while j < len(src):
                if src[j] == "{":
                    depth += 1
                elif src[j] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                elif src[j] == '"':
                    j = slint_string_end(src, j)
                    continue
                j += 1
        elif c == "\\":
            j += 1
        elif c == '"':
            return j + 1
        j += 1
    return len(src)


def interpolations(src, i, j):
    """The expressions of the string from `i` to `j`, as (start, end)
    of the text between `\\{` and its `}`."""
    out = []
    k = i + 1
    while k < j - 1:
        if src[k] == "\\" and src.startswith("{", k + 1):
            depth, m = 0, k + 1
            while m < j:
                if src[m] == "{":
                    depth += 1
                elif src[m] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                elif src[m] == '"':
                    m = slint_string_end(src, m)
                    continue
                m += 1
            out.append((k + 2, m))
            k = m + 1
        elif src[k] == "\\":
            k += 2
        else:
            k += 1
    return out


def lex_slint(src):
    toks = []
    i, line, n = 0, 1, len(src)
    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            i += 1
        elif c in " \t\r":
            i += 1
        elif src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            line += src.count("\n", i, j)
            i = j
        elif c == '"':
            j = slint_string_end(src, i)
            raw = src[i + 1:j - 1]
            inner = interpolations(src, i, j)
            if inner:
                # Its own words, quoted, and then the expressions inside
                # it, lexed: an `@tr` may be one of them.
                words, at = [], i + 1
                for a, b in inner:
                    words.append(src[at:a - 2])
                    at = b + 1
                words.append(src[at:j - 1])
                toks.append(Tok("str", '"' + " ".join(words) + '"', None, line))
                for a, b in inner:
                    for t in lex_slint(src[a:b]):
                        t.line += line - 1 + src.count("\n", i, a)
                        toks.append(t)
            else:
                toks.append(Tok("str", src[i:j], slint_unescape(raw), line))
            line += raw.count("\n")
            i = j
        elif c.isalpha() or c == "_" or c == "@":
            m = re.match(r"@?[A-Za-z_][A-Za-z0-9_-]*", src[i:i + 256])
            text = m.group(0)
            # A trailing `-` is subtraction, not the name's.
            text = text.rstrip("-")
            toks.append(Tok("ident", text, None, line))
            i += len(text)
        elif c.isdigit():
            m = re.match(r"[0-9][0-9A-Za-z_.%]*", src[i:i + 256])
            toks.append(Tok("num", m.group(0), None, line))
            i += m.end()
        else:
            p = next((p for p in PUNCT2 if src.startswith(p, i)), c)
            toks.append(Tok("punct", p, None, line))
            i += len(p)
    return toks


# ------------------------------------------------------- the source's marks


class Mark:
    """A string the source marks for the catalog."""

    def __init__(self, ctx, msgid, plural, where):
        self.ctx, self.msgid, self.plural, self.where = ctx, msgid, plural, where

    def key(self):
        return (self.ctx, self.msgid)


def close_of(toks, k):
    """The index of the bracket closing the one at `k`."""
    depth = 0
    for j in range(k, len(toks)):
        t = toks[j].text
        if toks[j].kind == "punct" and t in "([{":
            depth += 1
        elif toks[j].kind == "punct" and t in ")]}":
            depth -= 1
            if depth == 0:
                return j
    return len(toks) - 1


def read_mark(toks, k, path, line=None):
    """The message whose arguments open at the bracket `k`: `"s"`,
    `"ctx" => "s"`, either with `| "plural" % n`; None if the first
    argument is no literal (a macro's own definition). Placed at `line`,
    else at its English's line, as xtr places it."""
    a = toks[k + 1:k + 6]
    if not a or a[0].kind != "str" or a[0].value is None:
        return None
    ctx = None
    if len(a) > 2 and a[1].text == "=>" and a[2].kind == "str":
        ctx, a = a[0].value, a[2:]
    plural = None
    if len(a) > 2 and a[1].text == "|" and a[2].kind == "str":
        plural = a[2].value
    return Mark(ctx, a[0].value, plural, f"{rel(path)}:{line or a[0].line}")


def rust_files():
    out = []
    for d, _, files in os.walk(os.path.join(ROOT, RUST_DIR)):
        out += [os.path.join(d, f) for f in files if f.endswith(".rs")]
    return sorted(out)


def slint_files():
    out = []
    for d, _, files in os.walk(os.path.join(ROOT, SLINT_DIR)):
        out += [os.path.join(d, f) for f in files if f.endswith(".slint")]
    return sorted(out)


def rel(path):
    return os.path.relpath(path, ROOT).replace(os.sep, "/")


def source_marks(sources):
    marks = []
    for path, kind, toks in sources:
        for k, t in enumerate(toks):
            if kind == "slint":
                if t.text == "@tr" and k + 1 < len(toks) and toks[k + 1].text == "(":
                    m = read_mark(toks, k + 1, path, t.line)
                    if m:
                        marks.append(m)
            elif (t.kind == "ident" and t.text == "tr" and k + 2 < len(toks)
                  and toks[k + 1].text == "!" and toks[k + 2].text in "([{"
                  and not (k > 0 and toks[k - 1].text == "macro_rules")):
                m = read_mark(toks, k + 2, path)
                if m:
                    marks.append(m)
    return marks


# ---------------------------------------------------------------- catalogs


def po_unescape(s):
    out = []
    i = 0
    while i < len(s):
        c = s[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        n = s[i + 1]
        if n in "01234567":
            m = re.match(r"[0-7]{1,3}", s[i + 1:])
            out.append(chr(int(m.group(0), 8)))
            i += 1 + m.end()
            continue
        if n == "x":
            m = re.match(r"[0-9a-fA-F]+", s[i + 2:])
            out.append(chr(int(m.group(0), 16)))
            i += 2 + m.end()
            continue
        out.append({"n": "\n", "t": "\t", "r": "\r", "a": "\a", "b": "\b",
                    "f": "\f", "v": "\v"}.get(n, n))
        i += 2
    return "".join(out)


class Entry:
    def __init__(self, line):
        self.line = line
        self.ctx = None
        self.msgid = None
        self.plural = None
        self.strs = {}
        self.flags = set()
        self.obsolete = False

    def key(self):
        return (self.ctx, self.msgid)

    def translated(self):
        return bool(self.strs) and all(self.strs.values())


def read_po(path):
    entries = []
    cur = None
    field = None
    with open(path, encoding="utf-8-sig") as f:
        lines = f.read().split("\n")

    def flush():
        nonlocal cur
        if cur is not None and cur.msgid is not None:
            entries.append(cur)
        cur = None

    for no, raw in enumerate(lines, 1):
        line = raw.strip()
        obsolete = line.startswith("#~")
        if obsolete:
            line = line[2:].strip()
            if line.startswith("|"):
                continue
        if not line:
            flush()
            continue
        if line.startswith("#"):
            if cur is not None and cur.msgid is not None and field is not None:
                flush()
            if cur is None:
                cur = Entry(no)
            if line.startswith("#,"):
                cur.flags |= {f.strip() for f in line[2:].split(",")}
            field = None
            continue
        m = re.match(r'(msgctxt|msgid_plural|msgid|msgstr(?:\[(\d+)\])?)\s+"(.*)"$', line)
        if m:
            name = m.group(1)
            if name in ("msgctxt", "msgid") and cur is not None and cur.msgid is not None:
                flush()
            if cur is None:
                cur = Entry(no)
            cur.obsolete = cur.obsolete or obsolete
            value = po_unescape(m.group(3))
            if name == "msgctxt":
                cur.ctx, field = value, "ctx"
            elif name == "msgid":
                cur.msgid, field = value, "msgid"
            elif name == "msgid_plural":
                cur.plural, field = value, "plural"
            else:
                field = int(m.group(2) or 0)
                cur.strs[field] = value
            continue
        m = re.match(r'"(.*)"$', line)
        if m and cur is not None and field is not None:
            value = po_unescape(m.group(1))
            if field == "ctx":
                cur.ctx += value
            elif field == "msgid":
                cur.msgid += value
            elif field == "plural":
                cur.plural += value
            else:
                cur.strs[field] += value
            continue
        raise SystemExit(f"{rel(path)}:{no}: cannot read {raw!r}")
    flush()
    return [e for e in entries if e.msgid != "" or e.ctx is not None]


# ------------------------------------------------------------ placeholders


def placeholders(s):
    """The values a template asks for, as `fill` in i18n.rs reads it:
    `{}` the next by place, `{0}` by place, `{name}` by name; `{{` and
    `}}` are braces. Slint's `@tr` reads `{}`, `{0}`, and `{n}` alike."""
    found = set()
    nxt = 0
    i = 0
    while i < len(s):
        if s.startswith("{{", i) or s.startswith("}}", i):
            i += 2
            continue
        if s[i] == "{":
            close = s.find("}", i)
            if close < 0:
                break
            name = s[i + 1:close]
            if name == "":
                found.add(str(nxt))
                nxt += 1
            else:
                found.add(name)
            i = close + 1
            continue
        i += 1
    return found


def placeholder_problem(e):
    """What is wrong with a translated entry's placeholders, or None."""
    one = placeholders(e.msgid)
    if e.plural is None:
        got = placeholders(e.strs.get(0, ""))
        lost, extra = one - got, got - one
    else:
        many = placeholders(e.plural)
        lost, extra = set(), set()
        for s in e.strs.values():
            got = placeholders(s)
            # A form may leave out the count, as English's "One frame".
            lost |= (one & many) - got - {"n"}
            extra |= got - (one | many)
    if not lost and not extra:
        return None
    said = []
    if lost:
        said.append("lacks " + ", ".join("{" + p + "}" for p in sorted(lost)))
    if extra:
        said.append("has " + ", ".join("{" + p + "}" for p in sorted(extra))
                    + " the English does not")
    return "; ".join(said)


# ------------------------------------------------- words nothing marks


def read_allow(path):
    """Quoted strings let through anywhere, and `set_name` or `name:`
    lines naming a Rust setter or a Slint property whose strings are
    keys, never shown."""
    strings, names = set(), set()
    if not os.path.exists(path):
        return strings, names
    with open(path, encoding="utf-8") as f:
        for no, line in enumerate(f, 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith('"') and line.endswith('"') and len(line) >= 2:
                # As written, and with its escapes read (`\n`, `\u{b0}`).
                strings |= {line[1:-1], rust_unescape(line[1:-1])}
            elif re.fullmatch(r"[A-Za-z_][A-Za-z0-9_-]*:?", line):
                names.add(line.rstrip(":"))
            else:
                raise SystemExit(f"{rel(path)}:{no}: neither a quoted string nor a name: {line!r}")
    return strings, names


def wordy(s):
    """Whether `s` has a word in it, outside its placeholders: a string
    of glyphs, numbers, or values alone is not."""
    return s is not None and WORD.search(re.sub(r"\{[^{}]*\}", "", s)) is not None


def test_regions(toks):
    """Token ranges under `#[cfg(test)]` or `#[test]`, and the names of
    modules declared `#[cfg(test)] mod name;` (files of tests)."""
    regions, test_mods = [], set()
    k = 0
    while k < len(toks):
        if toks[k].text == "#" and k + 1 < len(toks) and toks[k + 1].text == "[":
            end = close_of(toks, k + 1)
            attr = " ".join(t.text for t in toks[k + 2:end])
            if attr in ("test", "cfg ( test )") or attr.startswith("cfg ( all ( test"):
                # The item: to its `;`, or its body's closing brace.
                j = end + 1
                while j < len(toks) and toks[j].text not in ("{", ";"):
                    if toks[j].text == "#" and toks[j + 1].text == "[":
                        j = close_of(toks, j + 1)
                    j += 1
                if j < len(toks) and toks[j].text == ";":
                    if toks[j - 2].text == "mod":
                        test_mods.add(toks[j - 1].text)
                    regions.append((k, j))
                elif j < len(toks):
                    regions.append((k, close_of(toks, j)))
                    k = close_of(toks, j)
            k = end + 1
            continue
        k += 1
    return regions, test_mods


def rust_candidates(path, toks, allow, names, skip_file):
    """Strings with letters handed to the window outside `tr!`: an
    argument of a `set_` call or of `SharedString::from`."""
    if skip_file:
        return []
    regions, _ = test_regions(toks)
    in_test = [False] * len(toks)
    for a, b in regions:
        for j in range(a, min(b + 1, len(toks))):
            in_test[j] = True
    found = []
    # The open brackets around each token: (index, what opened it).
    stack = []
    for k, t in enumerate(toks):
        if t.kind == "punct" and t.text in "([{":
            opener = None
            if t.text == "(" and k > 0:
                p = toks[k - 1]
                if p.text == "!" and k > 1:
                    opener = ("macro", toks[k - 2].text)
                elif p.kind == "ident":
                    is_from = (p.text == "from" and k > 2 and toks[k - 2].text == "::"
                               and toks[k - 3].text == "SharedString")
                    opener = ("from", p.text) if is_from else ("call", p.text)
            elif t.text in "[{" and k > 1 and toks[k - 1].text == "!":
                opener = ("macro", toks[k - 2].text)
            stack.append(opener)
            continue
        if t.kind == "punct" and t.text in ")]}":
            if stack:
                stack.pop()
            continue
        if t.kind != "str" or in_test[k] or not wordy(t.value) or t.value in allow:
            continue
        openers = [o for o in stack if o]
        if any(o[0] == "macro" and (o[1] == "tr" or o[1] in NOT_SHOWN_MACROS) for o in openers):
            continue
        if any(o[0] == "call" and o[1] in NOT_SHOWN_CALLS for o in openers):
            continue
        # Compared, never shown.
        before = toks[k - 1].text if k else ""
        after = toks[k + 1].text if k + 1 < len(toks) else ""
        if before in ("==", "!=") or after in ("==", "!=") or before == "=>" or after == "=>":
            continue
        feeds = [o for o in openers if o[0] == "from" or (o[0] == "call" and o[1].startswith("set_"))]
        if not feeds:
            continue
        setter = next((o[1] for o in reversed(openers) if o[0] == "call" and o[1].startswith("set_")),
                      "SharedString::from")
        if setter in names:
            continue
        found.append((rel(path), t.line, t.value, setter))
    return found


def compared_strings(sources):
    """Every string that is an operand of `==` or `!=` anywhere, in
    Slint or Rust, or a match arm's pattern: a key, compared, wherever
    else it is set."""
    out = set()
    for _, _, toks in sources:
        for k, t in enumerate(toks):
            if t.kind != "str" or t.value is None:
                continue
            before = toks[k - 1].text if k else ""
            after = toks[k + 1].text if k + 1 < len(toks) else ""
            if (before in ("==", "!=") or after in ("==", "!=", "=>")
                    or (after == "|" and before in ("|", "{", ","))):
                out.add(t.value)
    return out


def keyed_properties(sources):
    """Slint properties that hold a key: bound to a `Segmented`'s or a
    `ComboBox`'s `current` or `options` (the shown words are its
    `labels`), and the strings written in an `options` list."""
    names, strings = set(), set()
    for _, kind, toks in sources:
        if kind != "slint":
            continue
        for k, t in enumerate(toks):
            if t.text not in ("current", "current-value", "options") or k + 1 >= len(toks):
                continue
            if toks[k + 1].text not in (":", "<=>"):
                continue
            j = k + 2
            if toks[j].text == "root" and toks[j + 1].text == ".":
                names.add(toks[j + 2].text)
            elif toks[j].text == "[" and t.text == "options":
                for u in toks[j:close_of(toks, j)]:
                    if u.kind == "str" and u.value is not None:
                        strings.add(u.value)
    return names, strings


def slint_candidates(path, toks, allow, names):
    """Strings with letters bound outside `@tr` to a property the
    Slint test does not read: `name: "..."`, a property's default, a
    struct field, or an array item, not compared and not an argument of
    a callback (a key)."""
    found = []
    k = 0
    n = len(toks)
    while k < n:
        t = toks[k]
        # `name: <expr>;` and `property <T> name: <expr>;`
        if not (t.kind == "ident" and k + 1 < n and toks[k + 1].text == ":"
                and not (k > 0 and toks[k - 1].text in (".", "<"))):
            k += 1
            continue
        name = t.text
        j = k + 2
        depth = 0
        tr_depths = []
        call_depths = []
        while j < n:
            u = toks[j]
            if u.kind == "punct" and u.text in "([{":
                depth += 1
                if u.text == "(" and j > 0 and toks[j - 1].text == "@tr":
                    tr_depths.append(depth)
                elif u.text == "(" and j > 0 and toks[j - 1].kind == "ident":
                    call_depths.append(depth)
            elif u.kind == "punct" and u.text in ")]}":
                if depth == 0:
                    break
                if tr_depths and tr_depths[-1] == depth:
                    tr_depths.pop()
                if call_depths and call_depths[-1] == depth:
                    call_depths.pop()
                depth -= 1
            elif u.kind == "punct" and u.text in ";," and depth == 0:
                break
            elif u.kind == "punct" and u.text == ":" and depth == 0:
                break  # the next binding, in a struct literal
            elif u.kind == "str" and not tr_depths and not call_depths:
                before = toks[j - 1].text
                after = toks[j + 1].text if j + 1 < n else ""
                compared = before in ("==", "!=") or after in ("==", "!=")
                if (not compared and name not in SCANNED and name not in names
                        and wordy(u.value if u.value is not None else u.text)
                        and u.value not in allow and u.text[1:-1] not in allow):
                    found.append((rel(path), u.line, u.value if u.value is not None else u.text[1:-1], name))
            j += 1
        k = j
    return found


# --------------------------------------------------------------------- main


def self_test():
    """The readers and the placeholder rule on small cases of each
    shape the source and the catalogs use."""
    def marks(path, kind, src):
        lex = lex_slint if kind == "slint" else lex_rust
        return [(m.ctx, m.msgid, m.plural, m.where) for m in source_marks([(path, kind, lex(src))])]

    slint = (
        'Text { text: @tr("Plain \\"q\\" \\u{00b0}"); }\n'
        'Text { text: @tr("zoom" => "Fit"); }\n'
        'Text { text: @tr("{n} frame" | "{n} frames" % n); }\n'
        'Text { text: "\\{@tr("Inside {}", count)} more"; }\n'
        'Text { text: "a\\{root.n > 1 ? @tr("Nested") : ""}b"; } // @tr("Comment")\n'
        '/* @tr("Block") */\n'
    )
    got = marks(os.path.join(ROOT, "x.slint"), "slint", slint)
    assert got == [
        (None, 'Plain "q" °', None, "x.slint:1"),
        ("zoom", "Fit", None, "x.slint:2"),
        (None, "{n} frame", "{n} frames", "x.slint:3"),
        (None, "Inside {}", None, "x.slint:4"),
        (None, "Nested", None, "x.slint:5"),
    ], got

    rust = (
        "let c = '\"'; let l: &'a str = tr!(\"ctx\" => \"{n} x\" | \"{n} xs\" % n, w = r#\"q\"#);\n"
        "// tr!(\"no\")\n/* tr!(\"no2\") /* nested */ */ marked::tr!(\"M\");\n"
        "macro_rules! tr { ($m:literal) => { $crate::tr!($m) } }\n"
        "let s = tr!(\n    \"Long \\\n     line {}\",\n    a\n);\n"
    )
    got = marks(os.path.join(ROOT, "x.rs"), "rust", rust)
    assert got == [
        ("ctx", "{n} x", "{n} xs", "x.rs:1"),
        (None, "M", None, "x.rs:3"),
        (None, "Long line {}", None, "x.rs:6"),
    ], got

    def entry(msgid, msgstr, plural=None, strs=None):
        e = Entry(1)
        e.msgid, e.plural, e.strs = msgid, plural, strs or {0: msgstr}
        return e

    for e, want in (
        (entry("{} of {}", "{1} sur {0}"), None),
        (entry("{} of {}", "{} sur"), "lacks {1}"),
        (entry("{done} of {total}", "{total}, {done}"), None),
        (entry("{done} of {total}", "{totl}, {done}"), "lacks {total}; has {totl} the English does not"),
        (entry("{{x}} {a}", "{{y}} {a}"), None),
        (entry("{n} frame", "", "{n} frames", {0: "une vue", 1: "{n} vues"}), None),
        (entry("{n} in {w}", "", "{n} in {w}", {0: "{n} dans", 1: "{n} dans {w}"}), "lacks {w}"),
        (entry("Done", "Fait {x}"), "has {x} the English does not"),
    ):
        assert placeholder_problem(e) == want, (e.msgid, placeholder_problem(e), want)

    import tempfile
    po = (
        'msgid ""\nmsgstr ""\n"Language: fr\\n"\n\n'
        '#. A note.\n#: a.rs:1\nmsgctxt "zoom"\nmsgid "Fit"\nmsgstr "Adapter"\n\n'
        '#, fuzzy\nmsgid "Long "\n"one"\nmsgstr "Longue"\n\n'
        'msgid "{n} frame"\nmsgid_plural "{n} frames"\nmsgstr[0] "{n} vue"\nmsgstr[1] ""\n\n'
        '#~ msgid "Gone"\n#~ msgstr "Parti"\n'
    )
    with tempfile.NamedTemporaryFile("w", suffix=".po", delete=False, encoding="utf-8") as f:
        f.write(po)
    try:
        es = read_po(f.name)
    finally:
        os.unlink(f.name)
    got = [(e.ctx, e.msgid, e.plural, e.obsolete, "fuzzy" in e.flags, e.translated()) for e in es]
    assert got == [
        ("zoom", "Fit", None, False, False, True),
        (None, "Long one", None, False, True, True),
        (None, "{n} frame", "{n} frames", False, False, False),
        (None, "Gone", None, True, False, True),
    ], got
    print("self-test: passed")


def main():
    ap = argparse.ArgumentParser(description="Check the translators' catalogs against the source.")
    ap.add_argument("--list", action="store_true", help="list every reported string, not only the counts")
    ap.add_argument("--self-test", action="store_true", help="test the readers on small cases, then stop")
    args = ap.parse_args()
    out = sys.stdout
    if args.self_test:
        self_test()
        return 0

    sources = []
    for path in slint_files():
        with open(path, encoding="utf-8") as f:
            sources.append((path, "slint", lex_slint(f.read())))
    for path in rust_files():
        with open(path, encoding="utf-8") as f:
            sources.append((path, "rust", lex_rust(f.read())))

    errors = 0

    # 1. The template against the source.
    marks = source_marks(sources)
    pot = read_po(os.path.join(ROOT, POT))
    pot_keys = {e.key(): e for e in pot if not e.obsolete}
    marked = {}
    for m in marks:
        marked.setdefault(m.key(), m)
    missing = [m for k, m in marked.items() if k not in pot_keys]
    stale = [e for k, e in pot_keys.items() if k not in marked]
    plural_off = [(m, pot_keys[m.key()]) for m in marked.values()
                  if m.key() in pot_keys and pot_keys[m.key()].plural != m.plural]
    # One message marked with and without a plural, or with two plurals.
    split = defaultdict(set)
    for m in marks:
        split[m.key()].add(m.plural)
    torn = [k for k, p in split.items() if len(p) > 1]

    def says(ctx, msgid, plural=None):
        s = repr(msgid) if plural is None else f"{msgid!r} | {plural!r}"
        return s if ctx is None else f"{ctx!r} => {s}"

    print(f"{POT}: {len(pot_keys)} entries; the source marks {len(marked)} strings", file=out)
    for m in missing:
        print(f"  error: {m.where}: {says(m.ctx, m.msgid, m.plural)} has no entry in {POT}; "
              f"add one by hand after its file's neighbors, or run scripts/translations.sh", file=out)
    for e in stale:
        print(f"  error: {POT}:{e.line}: {says(e.ctx, e.msgid, e.plural)} is marked nowhere in the source; "
              f"remove the entry, or run scripts/translations.sh", file=out)
    for m, e in plural_off:
        print(f"  error: {m.where}: {says(m.ctx, m.msgid, m.plural)} is "
              f"{says(e.ctx, e.msgid, e.plural)} at {POT}:{e.line}; make the entry's plural the source's",
              file=out)
    for k in torn:
        print(f"  error: {says(*k)} is marked with differing plurals; one message has one English plural",
              file=out)
    errors += len(missing) + len(stale) + len(plural_off) + len(torn)

    # 2. Each language against the template.
    tdir = os.path.join(ROOT, "translations")
    for lang in sorted(os.listdir(tdir)):
        path = os.path.join(tdir, lang, "LC_MESSAGES", "greycard.po")
        if not os.path.isfile(path):
            continue
        po = read_po(path)
        live = {e.key(): e for e in po if not e.obsolete}
        lacking = [e for k, e in pot_keys.items() if k not in live]
        unknown = [e for k, e in live.items() if k not in pot_keys]
        kept_obsolete = [e for e in po if e.obsolete]
        fuzzy = [e for e in live.values() if "fuzzy" in e.flags and e.key() in pot_keys]
        empty = [e for e in live.values() if e.key() in pot_keys and "fuzzy" not in e.flags
                 and not e.translated()]
        done = sum(1 for e in live.values() if e.key() in pot_keys and "fuzzy" not in e.flags
                   and e.translated())
        # A slip in an entry the template no longer has is never read:
        # reported, not failed.
        broken, broken_unused = [], []
        for e in live.values():
            if "fuzzy" in e.flags or not e.translated():
                continue
            why = placeholder_problem(e)
            if why:
                (broken if e.key() in pot_keys else broken_unused).append((e, why))
        print(f"{rel(path)}: {done} of {len(pot_keys)} translated; {len(lacking)} missing, "
              f"{len(empty)} untranslated, {len(fuzzy)} fuzzy, "
              f"{len(unknown) + len(kept_obsolete)} obsolete "
              f"({len(unknown)} no longer in the template, {len(kept_obsolete)} marked #~)", file=out)
        for e, why in broken:
            print(f"  error: {rel(path)}:{e.line}: {says(e.ctx, e.msgid, e.plural)}: the translation {why}; "
                  f"the translator keeps every placeholder of the English (docs/translating.md)", file=out)
        for e, why in broken_unused:
            print(f"  unused: {rel(path)}:{e.line}: {says(e.ctx, e.msgid, e.plural)}: the translation {why}",
                  file=out)
        errors += len(broken)
        if args.list:
            for title, items in (("missing", lacking), ("untranslated", empty), ("fuzzy", fuzzy),
                                 ("no longer in the template", unknown)):
                for e in items:
                    where = f"{POT}:{e.line}" if title == "missing" else f"{rel(path)}:{e.line}"
                    print(f"  {title}: {where}: {says(e.ctx, e.msgid, e.plural)}", file=out)

    # 3. Words nothing marks.
    allow, names = read_allow(os.path.join(ROOT, ALLOW))
    slint_allow, _ = read_allow(os.path.join(ROOT, SLINT_ALLOW))
    test_files = set()
    for path, kind, toks in sources:
        if kind == "rust":
            _, mods = test_regions(toks)
            base = os.path.splitext(path)[0]
            parent = os.path.dirname(path) if os.path.basename(path) in ("mod.rs", "lib.rs", "main.rs") else base
            for m in mods:
                test_files.add(os.path.join(parent, m + ".rs"))
                test_files.add(os.path.join(parent, m, "mod.rs"))
    keyed, options = keyed_properties(sources)
    allow = allow | compared_strings(sources) | options
    names = names | keyed | {"set_" + n.replace("-", "_") for n in keyed}
    found = []
    for path, kind, toks in sources:
        if kind == "slint":
            found += slint_candidates(path, toks, allow | slint_allow, names)
        else:
            skip = path in test_files or os.path.basename(path) == "testing.rs"
            found += rust_candidates(path, toks, allow, names, skip)
    by_file = defaultdict(list)
    for f in found:
        by_file[f[0]].append(f)
    n_slint = sum(1 for f in found if f[0].endswith(".slint"))
    print(f"unmarked: {len(found)} candidates ({n_slint} in Slint, {len(found) - n_slint} in Rust) "
          f"in {len(by_file)} files; {ALLOW} lists the data and keys", file=out)
    if args.list:
        for file in sorted(by_file):
            print(f"  {file}", file=out)
            for _, line, text, via in by_file[file]:
                print(f"    {line}: {text!r} ({via})", file=out)

    if errors:
        print(f"{errors} errors", file=out)
        return 1
    if not args.list:
        print("no errors; --list for every reported string", file=out)
    return 0


if __name__ == "__main__":
    sys.exit(main())

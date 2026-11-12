#!/bin/sh
# Refresh the translators' catalogs from the source: the window's
# strings in the Slint files (@tr) and in the Rust (tr!), into
# translations/greycard.pot, and every language's
# translations/<lang>/LC_MESSAGES/greycard.po merged with it.
#
#     scripts/translations.sh
#
# Needs slint-tr-extractor and xtr (cargo install slint-tr-extractor
# xtr), and GNU gettext's msgcat, msgmerge, and msgfmt. A tool not on
# the PATH is looked for in target/tools/bin, where
# `cargo install --root target/tools` puts it.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
PATH="$PATH:$root/target/tools/bin"

missing=""
for tool in slint-tr-extractor xtr msgcat msgmerge msgfmt; do
    command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
if [ -n "$missing" ]; then
    echo "translations.sh: missing:$missing" >&2
    echo "  slint-tr-extractor and xtr: cargo install slint-tr-extractor xtr" >&2
    echo "  msgcat, msgmerge, and msgfmt: GNU gettext (the gettext package)" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The Slint side, with no default context: the window's build bundles
# the catalogs the same way, so one string is one entry wherever it
# appears. Sorted, so the catalog does not move with the file system.
find crates/greycard-slint/ui -name '*.slint' | LC_ALL=C sort > "$work/slint-files"
# shellcheck disable=SC2046
slint-tr-extractor --no-default-translation-context -d greycard \
    -o "$work/slint.pot" $(cat "$work/slint-files")

# The Rust side: the library and the binary, each walked from its root
# through its modules.
xtr --package-name greycard -o "$work/lib.pot" crates/greycard-ui/src/lib.rs
xtr --package-name greycard -o "$work/bin.pot" crates/greycard-ui/src/main.rs
# xtr resolves a module declared in a non-mod.rs file (archive.rs's
# `mod rejects`) beside its parent, not under it, so the file it should
# read is never read: each file with a tr! that neither walk reached is
# read as a root of its own.
: > "$work/more.pot"
for rs in $(grep -rl --include='*.rs' 'tr!' crates/greycard-ui/src | LC_ALL=C sort); do
    grep -q "[ ]$rs:" "$work/lib.pot" "$work/bin.pot" && continue
    xtr --package-name greycard -o "$work/one.pot" "$rs"
    [ -s "$work/one.pot" ] || continue
    if [ -s "$work/more.pot" ]; then
        msgcat --use-first -o "$work/more.pot" "$work/more.pot" "$work/one.pot"
    else
        cp "$work/one.pot" "$work/more.pot"
    fi
done

# One template: our header, then every string once, in the order of
# the files they are in.
cat > "$work/header.pot" <<'POT'
# The window's strings, from the Slint files and the Rust, for translators.
# Written by scripts/translations.sh; edit the language files, not this one.
# This file is distributed under the same license as greycard (GPL-3.0-or-later).
#
#, fuzzy
msgid ""
msgstr ""
"Project-Id-Version: greycard\n"
"Report-Msgid-Bugs-To: https://github.com/jessolmstead/greycard/issues\n"
"Language: \n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Plural-Forms: nplurals=2; plural=(n != 1);\n"
POT
msgcat --use-first --sort-by-file -o "$work/strings.pot" \
    "$work/slint.pot" "$work/lib.pot" "$work/bin.pot" \
    $( [ -s "$work/more.pot" ] && echo "$work/more.pot" )
msgcat --use-first -o translations/greycard.pot "$work/header.pot" "$work/strings.pot"

# Each language: new strings added untranslated, gone ones kept as
# obsolete (#~) for the translator, and a changed one marked fuzzy,
# which the builds leave in English until it is checked.
for po in translations/*/LC_MESSAGES/greycard.po; do
    [ -f "$po" ] || continue
    msgmerge --quiet --update --backup=none "$po" translations/greycard.pot
    echo "$po: $(msgfmt --statistics -o /dev/null "$po" 2>&1)"
done

# The template against the source, and every language's placeholders.
python3 scripts/translations-check.py

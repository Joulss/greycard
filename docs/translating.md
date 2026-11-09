# Translating greycard

greycard's window can be shown in any language there is a catalog
for. A catalog is one file, in gettext's `.po` format, that every
gettext editor opens (Poedit, Lokalize, GTranslator, or a text
editor). The builds carry the catalogs, so a new language reaches
users with the next release.

## Adding a language

1. Make `translations/<tag>/LC_MESSAGES/greycard.po`, where `<tag>`
   is the language's code: `de`, `es`, `ja`, or a language and region
   such as `pt_BR` when the region's wording differs. Either let
   gettext make it,

   ```sh
   msginit -l <tag> -i translations/greycard.pot \
       -o translations/<tag>/LC_MESSAGES/greycard.po
   ```

   or copy `translations/greycard.pot` there and delete the
   `#, fuzzy` line above its header.
2. Fill in the header: `Language: <tag>`, your name in
   `Last-Translator`, and the language's `Plural-Forms` line
   ([gettext's list](https://www.gnu.org/software/gettext/manual/html_node/Plural-forms.html)
   has every language's). French's is
   `nplurals=2; plural=(n > 1);`.
3. Translate. An entry left empty shows in English, so a catalog can
   be sent half done. An entry marked fuzzy also shows in English
   until the mark is cleared.
4. Build and run greycard, pick the language under Settings >
   Interface > Language, and start it again to see the window in it.
5. Open a pull request with the one file. If your language's name in
   itself is not yet in the Language list, say so in the pull request.

To update a catalog after the window's wording changed, run
`scripts/translations.sh` (see [building.md](building.md)) or ask in
the pull request: new strings arrive empty and changed ones fuzzy.

## Placeholders

A `{}` in a string is a value the window fills in, in order;
`{name}` is one by name, which a translation may move:

```po
msgid "Exporting {done} of {total}"
msgstr "Sur {total}, {done} exportées"
```

Keep every placeholder of the English, spelled the same. A `{}` that
must change place in your language can be written `{0}`, `{1}` (the
first value, the second): `"{} of {}"` may be `"{1} sur {0}"`. A
doubled brace, `{{` or `}}`, is a brace.

## Plurals

A string with a count has an English singular and plural and one form
for each of your language's plural forms, chosen by the header's rule:

```po
msgid "{n} frame"
msgid_plural "{n} frames"
msgstr[0] "{n} vue"
msgstr[1] "{n} vues"
```

`{n}` is the count. Fill every form: a plural entry with one form
left empty shows in English on one side of the window and empty on
the other.

## What stays English

The log, the command line's messages, the user guide, and the release
notes stay English. So do names that are data rather than words: the
program's name, a file's or a camera's name, the keys' names, and
units' symbols. Dates and numbers keep their English form for now.

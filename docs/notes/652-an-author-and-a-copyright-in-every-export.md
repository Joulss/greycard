# §652. An author and a copyright in every export (2026-11-12)

**Asked.** A tester wants exports to carry an author and a copyright
notice, set once and written into every file. The roadmap's item is
wider (the IPTC fields kept per frame in the sidecar, GPS); this is
its first half: the two defaults, the writers, and a way to set them
without the window.

**What exports did.** The camera's `Artist` and `Copyright` were
copied by rawler's `write_exif_tags` into the root directory, and
blank ones left out (§40). The XMP said nothing of either. Under
Metadata All and No edit they travelled; under None there was no EXIF
and no XMP, so they were stripped with everything else. A picture
input carries its own file's tags the same way. No export wrote
IPTC-IIM, so there is no IPTC to extend; the export formats have no
DNG, and the linear DNG and TIFF of `greycard develop` and the merges
keep the camera's tags, untouched.

**What we built.**

- Core: `exif::Rights` (author, copyright, each optional) on
  `Provenance`. Said, they go over the camera's in the EXIF and into
  the XMP as `dc:creator` (a `Seq`, one item a name, split on `;` as
  the Metadata Working Group has it; the EXIF keeps the text whole) and
  `dc:rights` (an `Alt` with `x-default`). `rights_payload` and
  `rights_xmp` are the two alone; `write_rgb16_tiff_tagged` takes
  `Tags::{Nothing, Provenance, Rights}`; `with_packet` puts any packet
  into a JPEG or PNG.
- Render: `export::Settings::rights`, written whatever `metadata`
  says. Not the sheet's and not a preset's: the sheet's `settings()`
  leaves it empty and the caller lays it on.
- Settings: `author` and `copyright` in `settings.json`, in `Shared`
  so the window and the command line read one pair. Empty is unset and
  left out of the file, so a file without them writes back byte for
  byte, and the released build passes over them as unknown keys; a
  value of the wrong kind reads as none, the rest standing. No schema
  bump.
- Reach: the headless plan lays them on (`greycard export`, its
  `--all`, the editor's `--export`); the window's `start_set` lays on
  the pair read at the start, which every window export takes: the
  sheet, Ctrl+E, the queue, Edit in.
- `greycard export --author NAME --copyright TEXT`, over the settings'
  for the run; a blank one is none for the run, the camera's standing.

**Precedence.** Per field: the flag, else the settings', else the
camera's when not blank. Nothing said leaves the file as it was, byte
for byte in the XMP.

**Privacy.** Under Metadata None the said author and copyright are
still written, alone: an EXIF of the two root tags and no EXIF
directory, an XMP of the two `dc:` properties. They are the user's own
statement, the issue asks for every export to carry them, and
Lightroom's narrowest option keeps the copyright too. The camera, the
time, the place, the source's name, and the edit stay out.

**Text.** Control characters (a newline, a tab, a NUL that would end
the EXIF string) become spaces, the ends are trimmed, and the text is
cut at 1024 bytes on a character's boundary; blank is unset, and an
author of only `;` is none. UTF-8 throughout: the EXIF's ASCII tags
carry the same bytes, as rawler's other strings do, and exiv2 and our
own picture reader give them back as written.

**Per format.**

| Format | EXIF | XMP |
|---|---|---|
| JPEG | APP1 `Exif` | APP1 packet |
| PNG | `eXIf` | `iTXt` packet |
| TIFF | root directory | tag 700 |
| WebP | `EXIF` chunk | `XMP ` chunk |

**exiv2.** `crates/greycard-cli/tests/export_rights.rs` (runs where
exiv2 is on the path, else says SKIPPED): the settings' copyright, the
flag's author over the settings', a source TIFF naming its own.

```
exiv2 -pa mine.jpg
Exif.Image.Artist      Ascii   24  Zoë Ångström; Jo Doe
Exif.Image.Copyright   Ascii   24  © 2026 Zoë Ångström
exiv2 -px mine.jpg
Xmp.dc.creator         XmpSeq   2  Zoë Ångström, Jo Doe
Xmp.dc.rights          LangAlt  1  lang="x-default" © 2026 Zoë Ångström
```

The TIFF prints the same four lines. With nothing said the source's
`Artist` and `Copyright` stand and `-px` lists no `Xmp.dc.`.

**What the window half needs.**

- Two text fields, Author and Copyright, on the Settings sheet (its
  Export category, or a new one if the IPTC fields will follow there);
  a hint under them that several authors take `;`. Their wording, and
  whether the copyright field offers a `©` and the year, are the
  maintainer's.
- Written to `settings.json` as typed (`Settings::change_at`), and
  `Window::rights` set again: today it is read once at the start, so a
  hand edit waits for a restart.
- Whether the export sheet says them (a line under Metadata, or in
  the summary the queue shows); `Settings::describe` leaves them out.
- The editor's `--export` takes no flags for them; it goes in a later
  release anyway.
- Later, the frame's own IPTC fields in the sidecar go over these
  defaults: frame, then settings, then the camera.

# §671. Olympus .ORI files and Gradation Auto (2026-11-14)

Issue 139.

## What was missing

An Olympus or OM System High Res Shot writes the merge as `NAME.ORF`
and one single frame as `NAME.ORI`, an ORF under another name.
`decode::RAW_EXTENSIONS` had no `ori`, so the browser, the index, the
kind filter, the pairs, the previews, the camera match's survey, the
import, and `--all` all passed the ORI over; the import left it on the
card. The decoder itself reads it by its content, as before.

An OM-1's Gradation Auto writes `0 -1 1 1` (exiv2: "Normal,
Auto-Override"). The reader took only a fourth value of 0, so an Auto
frame was not known and of no group: the camera match counted it with
the raws of no fixed style and never sampled it.

## What changed

- `ori` joins `RAW_EXTENSIONS`. Everything keyed on the list follows;
  the tests' own copies (the two `export_set` tests) now use it, and
  the ignored sample lists, the bench's training-data list, and the
  Windows register and unregister scripts take `.ori`.
- Gradation: a fourth value of 1 with a key we name (`0`, `-1`, `1`)
  is known and on, a tone setting, as High Key and Low Key are. Any
  other shape stays not known.
- An `.ORI` never owns, reads, or makes its stem's short names: the
  stem's `NAME.xmp`, its `NAME.pp3`, a Lightroom take from `NAME.xmp`,
  and the camera's `NAME.JPG`. It always yields, whether its `.ORF`
  is on disk or not (`decode::always_yields`, the half of
  `decode::short_names` that needs no listing); its own are its long
  names, `NAME.ORI.xmp`, `NAME.ORI.gcd`, and `NAME.ORI.pp3`. Ownership
  that flips with what is in the folder is what loses data: an ORI
  that took `NAME.xmp` while its ORF was away (moved to the rejects, or
  deleted outside greycard) would write over the ORF's, or take its
  rating and keywords. The ORF's side still looks at the listing: it
  owns the short names beside its ORI while it is there, and gone, they
  are nobody's. A RAF and its DNG stay as they were, neither's.
- An ORI is no rival for the others of its stem: a `.DNG` beside an
  `.ORI` and no `.ORF` owns `NAME.xmp`, as on master.
- The rule's readers with the real listing: our own XMP names
  (`xmp::stem_sharers`, so the open's adoption, `write_path`,
  `beside`, and `paths_of` for a move, a delete, the archive's copy,
  and the sync's write), Lightroom's take (`lightroom::beside`), the
  delete's shared XMP (`delete::shared_xmp`), and the rejects' check
  of a destination (`rejects::stem_owner`). RawTherapee's `NAME.pp3`,
  the pairs (`pairs::stem_key`, and through it the index's `pairkey`),
  and the import's scan ask only `always_yields`. The extension test
  inside the rule is private. Our XMP names answer an ORI before any
  listing, and `stem_sharers` reads a folder in one pass, a folded stem an entry, the
  rule asked of the stem's few names: `beside` over a folder of CR3s
  each with its short XMP, in a debug build, takes 2.6 s for 2,500 and
  49 s for 10,000, as master's does (2.6 s and 53 s). The index pass's
  own listing is not handed down to `beside`: it holds the frames
  only, not the XMPs or the other files of a stem, and `sidecar_of`
  is called from places that have no listing.
- The rejects' move never writes over a short XMP already at the
  destination beside a raw of the stem, as on master: what wrote it
  while the frame was away cannot be told from the names.
- The import's scan gave an XMP named for a whole raw (`P1.ORI.xmp`)
  to the stem's first raw (the ORF). It now goes to the frame it
  names. For ordinary files that changes three things: `E.JPG.xmp`
  beside a lone JPEG goes with it (left on the card before);
  `X.DNG.xmp` beside `X.RAF` and `X.DNG` goes with the DNG (with the
  RAF before); and an XMP whose inner extension is neither a raw nor
  a picture (`IMG.HIF.xmp` beside `IMG.ARW`) is left on the card.
  darktable's `IMG.JPG.xmp` for a raw's camera JPEG goes with that
  raw, as before, and one named for a raw that is not there
  (`X.CR2.xmp` beside `X.DNG`) goes with the stem's raw, as it did.
- RawTherapee's lookup is master's again: one set of stems a folder,
  spelled as the folder spells them, an ORI left out of it. Folding
  case is not needed for the ORI rule, which asks no listing, so
  `img.pp3` beside `img.jpg` and `IMG.CR3` on a disk that tells case
  apart is the JPEG's, as on master. A JPEG beside a lone ORI takes
  `NAME.pp3`, as on master, though it does not own `NAME.xmp` (the
  ORI is a file of its stem there).
- Index schema 16: the step from 15 marks every Olympus and OM System
  row with no style unread, so a library a master build made since
  the Olympus reader (schema 15, unreleased but on master since
  2026-11-12) re-reads an Auto frame into its group. An unknown
  Gradation always left the style NULL (`CameraStyle::group_key` is
  none when any setting is not known), so a row with a style read the
  tag as it reads now and is left alone.
- User guide: `.ORI` in the camera match's list of raws and limits,
  Gradation Auto in its tone settings, and a sentence in the library
  page's pairs section.

## The pair of an ORF and an ORI

- Sidecars: `NAME.ORF.gcd` and `NAME.ORI.gcd`, and the long XMP
  names; the short `NAME.xmp` and `NAME.pp3` are the ORF's while it is
  there, and go with it on a move or a delete. Never the ORI's.
- Previews and thumbnails are keyed by the content hash (head and
  size); the two files differ in both.
- Pairs: the JPEG is the ORF's, in any order and case; the ORI has no
  companion, beside its ORF or alone, so moving or deleting it takes
  nothing of the ORF's, and a JPEG beside a lone ORI is a frame of its
  own.
- Stacks, collections, and versions refer by hash and path; Forget
  missing and the archive work on rows and paths: two frames
  throughout. A burst may well propose the two as one stack (same
  second, same picture); that is a suggestion, and a fair one.
- Export of both to one folder: `NAME.jpg` and `NAME-1.jpg` under
  every policy, as for any two frames of one stem.

## What the real files showed

Six OM-1 frames from a tester (`OM 90mm F3.5`, Natural picture mode):
one frame each at Gradation Normal, High Key, Low Key, and Auto, and a
50 MP High Res pair (the `.ORF` merge and its `.ORI` single frame),
read with exiv2 and through the index:

| Frame | Gradation (0x050f) | Stacked Image (0x0804) | Index style | fixed |
|---|---|---|---|---|
| Normal | 0 -1 1 0 | 0 0 | Natural | yes |
| High Key | 1 -1 1 0 | 0 0 | Natural | no |
| Low Key | -1 -1 1 0 | 0 0 | Natural | no |
| Auto | 0 -1 1 1 | 0 0 | Natural (was none) | no |
| High Res merge (`.ORF`) | 0 -1 1 0 | 11 12 | none (a merge) | no |
| High Res single frame (`.ORI`) | 0 -1 1 0 | 0 0 | Natural | yes |

The ORI's maker note is the ORF's but for its sizes, preview length,
and Stacked Image: `0 0`, a plain frame, so it is fitted as one. Its
DriveMode (`0 0 0 0 4 36`) still says High Res Shot, as the ORF's
does; we do not read DriveMode. The ORF's `11 12` takes it out of every
group as a merge. The sample style test agrees with exiv2 on all six,
every field.

A folder of the High Res pair: the index lists two frames with two
hashes; each makes its own preview (2048x1536 each); a headless
`--export DIR --all` wrote the merge's develop (8160x6120) and the
single frame's (5184x3888) under the stem and the stem with `-1`, and
a `.gcd` beside each raw.

With a hand-written Lightroom XMP under the stem's short name beside
the pair (three stars, red, a stop up), an index pass took it for the
ORF alone: the ORF's row three stars and red with "Imported from
Lightroom" in its `.gcd`, the ORI's row bare and no `.gcd` written for
it. The six frames' index set back to schema 15, with the Auto frame
as a schema 15 build left it (no style, read), came up at 16 and the
next pass read the Auto frame into Natural.

The editor's delete path over copies of the High Res pair with a
stem XMP (`delete::tests::the_samples_orf_deleted_beside_its_ori`,
ignored, `GREYCARD_SAMPLES`): with the ORI there, deleting the ORF
took the XMP with it and left the ORI; with the ORF gone from the
disk first, the delete took nothing and the stem's XMP stayed.

RawTherapee's folder run over 2,500 and 10,000 raws each with its
`NAME.pp3` (`rawtherapee::tests::ten_thousand_profiles_take_four_times_two_and_a_half`,
ignored, debug build), master's set of stems with the ORI left out:
0.49 s and 1.42 s, a ratio of 2.9.

The lens warning is rawler's: its MFT table has no Olympus id
`00 39 10` (the lens's 0 57 16), so it falls back to the EXIF name.
lensfun has no `OM 90mm F3.5` either (our cached database and its
master today), so there is no lens correction for it on either side.

## What remains

- A culling tool's short `NAME.xmp` beside a lone ORI (no ORF ever
  there) is not read, and a Lightroom develop there is not taken: the
  cost of an ORI never owning its stem's short names. An import leaves
  such a `NAME.xmp` on the card.
- The Linux desktop entry and the Mac's Info.plist do not offer
  greycard for `.ori`: shared-mime-info has no type for it, and Apple's
  `com.olympus.raw-image` names `.orf`. Declaring our own types is a
  packaging decision.
- An import whose name pattern has `{seq}` gives the ORF and the ORI
  different numbers, so the pair no longer shares a stem after the
  copy; each keeps its companions. Left as it is.
- `photoshop:SidecarForExtension` is not read. Nothing in the repo
  shows what Lightroom writes there and no fixture carries it, and
  reading it in the Lightroom take alone would let the take and our
  XMP names disagree about whose `NAME.xmp` is. The rule decides.

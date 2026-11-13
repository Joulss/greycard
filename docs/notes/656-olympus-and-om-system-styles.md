# §656. Olympus and OM System styles for the camera match (2026-11-12)

The style reader (§180, §265) named an ORF's maker Other, so the camera
match left every Olympus and OM System frame out for want of a style
(issue 139). It now reads their maker note.

**What Olympus records.** An ORF's maker note sits in its Exif IFD under
one of three headers: "OM SYSTEM" (the OM Digital Solutions bodies,
16 bytes), "OLYMPUS\0II" (12 bytes; both count offsets from the note's
start), or the first bodies' "OLYMP\0" (8 bytes, offsets from the
file's start). The settings are in its Camera Settings sub-IFD
(0x2020). Tag numbers and names are exiv2's (`olympusmn_int.cpp`);
Tone Level (0x052e) and Stacked Image (0x0804), which exiv2 does not
name, follow ExifTool's `Olympus.pm`.

**What we read.**

- The style: Picture Mode (0x0520, first value) by exiv2's name. Vivid,
  Natural, Muted, Portrait, Sepia and Monotone are fixed. Monotone takes
  its filter (0x0525) and toning (0x0526) into its name when they are
  not Neutral, "Monotone + Red Filter + Sepia Tone", as a Fujifilm
  monochrome simulation takes its filter; one we cannot name leaves it
  unfixed. i-Enhance and e-Portrait adapt to the scene (as Canon's Auto
  does) and Color Creator and the Color and Monochrome Profiles are the
  user's own with no base the file names (as a Nikon custom control
  with no base): named, not fixed, of no group. Codes exiv2 does not
  name (ExifTool's 8 Underwater, 17 Art Mode) are `Unknown (n)`.
- Tone: Gradation (0x050f). "0 -1 1" Normal is off; Low Key and High
  Key are on and keep the frame in its group, not fixed; the camera's
  override (fourth value 1), a value exiv2 does not name, or no tag at
  all is not known. Highlight, shadow and midtone control (Tone Level)
  are listed only when moved, as Fujifilm's tones are.
- Replacing: an Art Filter (0x0529) or the compacts' Magic Filter
  (0x052c), a scene mode (0x0509, anything but Standard), and a stacked
  frame (0x0804 other than "0 0": in-camera HDR, Live ND, Live
  Composite, focus stack, high resolution). The filters and the stack
  tag absent are off, as on the bodies without them; a scene mode
  absent is not known.
- Vignetting: Shading Compensation (0x050c).
- Not read, as for the other makers: the Picture Mode's saturation,
  contrast, sharpness, hue and effect steps and the Custom Saturation,
  Contrast and Sharpness beside them. A group may mix them.

The group key starts "OM Digital Solutions" or "Olympus", the start of
the Make, so a look is named "OM Digital Solutions OM-1 Vivid".

**Existing libraries.** Schema 15 adds nothing and marks every row with
an Olympus or OM Digital Make and no maker unread, so the next pass over
its folder reads its tags and nothing else of the file.

**Tested.** Against exiv2 (`tests/camera_style.rs`) on five OM-1
frames, one a Picture Mode, two OM-5 frames, and an E-M10 Mark IV
(Make "OLYMPUS CORPORATION", the "OLYMPUS\0II" header): every field
agreed on every file. Five scrubbed OM-1 heads are fixtures. Checked on
synthetic notes only, since no sample has them: Gradation other than Normal
(and stored as SHORT), the filters, scene modes, stacks, tone levels
(and stored as SHORT), the Monotone names, the first bodies' "OLYMP\0"
header, a big-endian "OLYMPUS\0MM" note, and Camera Settings as an
UNDEFINED block. Through the match's own survey and plan, each OM-1
frame is its own group, fixed, and skipped only as one frame under the
twenty a fit needs. Gradation and Tone Level are read as 16-bit signed
whatever type the file declares, as exiv2 types them, so a body that
writes them as SHORT reads the same.

**Open.** How Gradation "Auto" is written: neither reference names
it. Written as anything but Normal it is not known and the frame is
left out; written as Normal it would pass as fixed. No sample has it.
The style reader has no fuzz target, for any maker.

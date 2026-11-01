# §474. A phone's DNG read as the phone meant it

*2026-10-31*

The first of the DNG tags the roadmap named, built against three
Pixel 9 files (the wide, the ultra-wide, and the front camera). rawler
parses none of them; the decoder reads the IFD entries it exposes.

**Gain maps.** OpcodeList2's GainMap, opcode 9, parsed from the
list's binary layout and applied to the normalized, cropped samples
before noise estimation and the demosaic: bilinear between the map's
points over the ActiveArea, measured at pixel centers, on the planes
and at the CFA pitch the map names, each result held between zero
and one as Adobe's SDK does (0.05 to 0.2 percent of samples held at
white on these files). An empty area means the whole image, as the
SDK reads it. Unknown opcodes and maps that will not parse are
logged and skipped, never an error; OpcodeList3's warps are logged
as not applied. The point count is a checked multiply: a crafted
triple whose product wraps to a small number passed the length check
and would have indexed past the buffer inside the develop's pool, a
crash in a release build; a fuzz target feeds the parser and the op
now. On the Pixels the maps are about 2.4x at the center and 3.6x at
the corners, so they carry a stop of brightness as well as the
shading; corner-to-center luminance went from 0.26 to 0.37 before to
0.76 to 0.87 after, against the phone's own 0.66 to 0.78.

**Forward matrices.** ForwardMatrix1 and 2 with their illuminants,
read from IFD 0 into the calibration the color code already took
when present; rawcolor blends them by temperature as it does the
color matrices.

**The baseline, carried and not yet applied.** BaselineExposure is on
the decoded frame in stops and logged once a decode. Applying it in
place of the per-body default was wrong: the tag is additive on
Adobe's scale, and our 0.8 was fitted against Lightroom renders that
already carried Adobe's value for the reference bodies, so an Adobe-
converted DNG of a Canon would have opened half a stop darker than
its CR3. The rule is our neutral (the body default less Adobe's
value for the reference bodies) plus the file's value, and the
neutral wants one measurement: an Adobe-converted DNG of an R5 II or
R6 II, its tag read. Until then every frame takes the body default,
the DCP's BaselineExposureOffset stays unread as before, and the
Lightroom import's conversion is off by the same difference once
the rule lands. The linear DNG writer keeps writing the headroom's
stops, and now the source's own tag too, so a re-saved phone file
keeps its value.

**What is left.** The rectilinear warps (a resample after the
demosaic, its place in the pipeline a question for the engine's
stage list), the bad-pixel list, the 3 MB ProfileGainTableMap a Pixel
carries (its local tone mapping, most of the remaining gap to the
phone's picture), and the neutral above. A Canon export is pixel-
identical before and after; cached thumbnails of DNGs with no
embedded preview are made again once.

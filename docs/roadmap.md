# Roadmap

The list to glance at and add to. Next is the tag about to be cut: one
sentence a tester can verify and the short list that makes it true,
chosen from the tracks. A tag with fixes only moves the patch number;
one with new features moves the minor. A track is a theme with a
sentence of its own as the test of when it is done, and a track's
sentence coming true is one reason for a minor bump. Everything
else waits in the pool by theme, and an item's "waits on" line decides
when it can join. Bugs sit at the top and belong to Next. Ticked items
move to `docs/changelog.md` under their version, so this file only
holds what is not done. One line an item, the notes section in
brackets, and for a blocked item what it waits on and nothing more;
the reasoning lives in `docs/notes.md`.

## Bugs

- [ ] The local contrast's GPU sweep fails one case in a thousand:
      seed 1, case 758, a 55x2547 Steps scene with texture -0.82,
      clarity -1, and structure 1, the GPU 1.3e-4 and the CPU 2.3e-4
      from the f64 port over the 5e-5 limit, on NVIDIA and lavapipe
      alike, on master before §553. The CPU reference is the one off;
      find the term that drifts on a tall thin scene
- [ ] The remove's UNet at a frame's first fill in the editor after a
      restart: 27 s and 40 s on the M5 before §482, whose bench loads
      them in 1.5 to 2.6 s from a warm cache. The window says
      "preparing..." while it loads and the load starts when the Fill
      is taken in hand (§490). Time the editor's own first fill, cold,
      before taking this off
- [ ] The window at rest holds a third of a laptop's GPU on Windows,
      in the Library after a frame was opened (an RTX 5060 Laptop at
      2.3x, discussion 51, 2026-10-29); the same path here draws
      nothing at rest at that scale. The log says since 9680ffd4 when
      the window keeps drawing, with what was on. A Windows log decides
- [ ] A Fast fill on a Windows GeForce RTX 3080 Ti reloads the models
      every fill (5 s each) and takes 11 to 19 s for 24 UNet calls
      against under 3 s here, and once lost the device after a 76 s
      model load, which the watchdog restarted on CPU ops (issue 54,
      2026-10-29); the slow path is still to find. The policy holds
      a discrete card to one at a time since the card's memory was
      not read; it is read now and logged. Tiers for a card are parked
      on the branch card-memory (12 GB UNet open, 20 GB decoder whole,
      24 GB all three): measured 2026-10-29 on a 16 GB card, the UNet
      kept open peaked at 13.1 GB over idle and held 11.5 GB after,
      since ORT's buffer cache on the shared context lives as long as
      the held UNet, so 12 GB cannot take it. Wants an A/B against one
      at a time with and without the learned denoiser, and the cache
      released or the held UNet dropped when another model loads; and
      the UNet is kept for one size only, so Fast fills of mixed sizes
      reload anyway
- [ ] System and File display profiles differ with the same profile
      picked (issue 53): waits on the reporter's profile; a table-based
      monitor profile is the lead
- [ ] "Finding people" ran forever and the masks broke after, on
      Windows (issue 44): waits on the reporter's log
- [ ] The Slint pin dropped once a release carries
      slint-ui/slint#14324 (§377)
- [ ] A straightened frame opened with a Linear mask shown panics at
      startup in lyon (`p.x.is_finite()`, through a Slint Path in
      femtovg's draw_path); the same sidecar unstraightened draws
      fine (found 2026-11-05 reviewing the constrained drags)
- [ ] A fill on a denoised frame still gets more grain than its
      surround after §434 (1.3x at every distance on an overcast sky
      with the learned denoiser, the amount not the shape): the
      estimate reads texture at the hole's level as noise once the
      real noise is below it. Cap it by the develop's mosaic noise
      model carried through the gains and the matrix, which the base
      does not carry today (§398). A wide draw is held to the
      shortfall's variance since §438; measuring the surround's
      variance within its quiet blocks, per level bin, would let a
      truly wide grain be drawn in full. And a fill whose own grain
      exceeds the surround's (3x with the profiled denoise) wants
      smoothing, not grain. A Windows tester sees the fill keep the
      frame's own noise under the learned denoiser (issue 63,
      2026-11-04); wants the raw. Harness in
      target/captures-2026-10-26/fill-grain

## Tracks

A track is done when its sentence is true. None is ordered before
another; the feel track's measured items come first within it.

### The feel

The default develop matches the camera JPEG's brightness and the
sliders feel like the ones people learned on.

Three of these are measurements with a target and go first: the
baseline exposure per body, the default brightness against the
embedded JPEG, and the slider response fitted to the reference set.
The rest are taste, and taste is not decided alone: each ships both
ways behind a switch, testers say which they keep, and then the
switch goes and the default follows. "Feels like Lightroom" means the
same direction and a similar size in the first third of the travel,
within a tenth of a stop; past that it is a look, and looks are the
Look section's job.

- [ ] A file opened from the Finder opens in the editor: the delegate's
      `application:openURLs:` is written but has not run on a Mac;
      try a double-click, Open With and a Dock drop, at launch and
      while it runs (§148)
- [ ] The default's saturation and the reds' skew, measured under AgX
      once it is the default (§230): the per-channel curve's slope
      added chroma in the mid-tones that read as over-saturated beside
      ACES 2.0 and AgX Punchy (§227), and its yellow skew of bright reds
      is what the eye expects where AgX holds them and they go pink.
      Measure the default develop's chroma by lightness band against
      the camera JPEG and a Lightroom export on the reference set, and
      the reds' hue the same way; Punchy's saturation and the camera
      match's fitted table are the two places to put the answer, not a
      skew of our own. Waits on the AgX port
- [ ] AgX's contrast and white as the feel track's controls (§231): the
      port ships Blender's slope over a range shortened to put white at
      the sensor's clip, about 19 percent more contrast per stop than
      Blender; a second construction keeps Blender's contrast per stop
      (`GREYCARD_AGX_SOFT`, no user control), and the user could not
      pick between them on 33 frames. Decide whether contrast, white
      and the inset's rotation (the reds' warmth, the lanterns) are
      sliders, a preset's, or fixed, by the track's rule. A Mac tester
      (2026-10-20) asks for the slope, the toe, the shoulder, the
      contrast, and the width exposed by hand; a pixls tester
      (2026-10-22) for the pivot and the toe and shoulder powers, as
      darktable's sigmoid has them
- [ ] OpenDRT (jedypod's open display transform) evaluated beside AgX
      as a second display transform, with its parameters, on the
      reference set; asked by the same tester
- [ ] Blender's sRGB and P3 guard rail lerps the luminance toward the
      opponent-compensated one by its 0.08 power; that step is Eary
      Chow's and his generator carries no license, so the port ships
      darktable's form of the rail, which departs from Blender's sRGB
      table by a 99th percentile of 18 levels on saturated colors
      inside Rec.2020 (§231). Waits on a license from its author, or on
      a derivation of our own from the method
- [ ] Reference frames for the feel: eight frames with a Lightroom edit
      each and the greycard values that match, a check of the default
      develop's brightness against the embedded JPEG, a slider response
      weighted to the first third of the travel, an Auto from the
      histogram (§85). Seven frames in and measured (§143); the
      baseline landed from them (§141). Still wanted: Lightroom bases
      for the forest, the torii, the pagoda and the flowers. Start from
      §150: the set, `tools/reference/`, where each control stands and
      what is open
- [ ] The baseline per body, after §348: the anchor is the user's call
      (the Canons pooled by session gives the GFX 100S II 0.52, equal
      weights 0.59, the R6 Mark II alone leaves it at 0.8, the camera
      JPEG itself takes every body down a quarter to half a stop:
      R5 0.76, R5 Mark II 0.54, R6 0.51, R6 Mark II 0.33, GFX 0.22,
      Z 6 III 0.66); the R6 Mark III and Z 6 III sit past the line by
      frame and want more sessions, the Z 6 III its High Efficiency
      raws; the two anchors read 0.21 apart by session, which §143
      also says, and whether the R6 Mark II's 0.8 is right is
      Lightroom's to say from history pairs of that body; Adobe's
      `BaselineExposure` from a DNG per body as the second yardstick;
      the fitted looks' own brightness (the GFX's 0.16 most); the
      import's offset on unedited frames (now 0.62 stops apart, §342)
- [ ] Whites and shadows reshaped against the set: whites moves only
      the top where Lightroom's moves the mid-tones, and wants two more
      frames of whites exports; shadows over-lifts the mid-tones of a
      low-key frame; the tops still sit 0.1 to 0.2 stops under
      Lightroom's where they are not the sensor's clip (§143, §145,
      §149, §150). The range decision is made (§315); wants the Lightroom
      whites exports
- [ ] The guided plane's halo at a hard edge, measured; darktable's
      exposure-independent guided filter if it shows (§85)
- [ ] The contrast filters weighted by chroma through the whole range,
      so a filtered sky keeps its gradient instead of going down as one
      flat band (§87). A change to the default look; waits on the
      reference frames
- [ ] The black and white base grey: Oklab lightness, where every other
      editor mixes the channels, so None starts flatter across hues than
      people expect (§87). A change to the default look and to every
      mono edit written; waits on the reference frames
- [ ] A reference view: a frame pinned beside the one being edited,
      the panel live and the reference untouchable, drawn from the
      developed preview the thumbnails keep so it costs no second
      develop; a frame menu entry and a key to pin the current frame,
      the compare's split, replaced by pinning another, cleared by the
      key again, kept across frame changes and not across runs. A
      0.9.0 item, after the modes have settled; the full version, the
      reference redeveloped at the viewport's size and following a
      proof, waits for someone to ask
- [ ] A menu bar: File, Edit, View, Frame and Help through Slint's
      MenuBar, which the winit backend hands to the native bar on
      macOS and Windows (muda, already in the build; SLINT_NO_MUDA
      turns it off for a snapshot) and draws in the window on Linux;
      each entry a callback the window already has, its chord shown
      from the keys table so the menus and the help sheet never
      drift; the app menu with Quit comes with the backend. Open:
      whether the drawn bar is kept on Linux and Windows or the
      MenuBar made conditional on a property, which Slint may not
      allow. The user's, for the laptop, 0.9.0
- [ ] The UI scale changed live, without a restart: the scale-changed
      event dispatched on the open window, then every cached size
      checked (the window fit, the strip's row heights, the viewport
      texture, the snapshot path), with the factor pinned in the
      screenshot tests. The §377 handler's dispatch is the machinery
- [ ] The Slint pin (§377) dropped once a release carries the logical
      size fix, as femtovg's is; the muda context menu's positions on
      Windows may carry the same mistake and are not in the patch. On
      a Mac at a Scale of 150% the menus open at the pointer
      (2026-10-31), whatever the code's reading said
- [ ] The sort shown as a labeled control: today it is the arrows at
      the end of the filter's row, and a tester who found a shoot in
      reverse order found no way to turn it. Asked 2026-10-18
- [ ] Adding to and taking from an AI mask made obvious: a mask holds
      shapes with Add and Subtract, so a brush stroke can add to a Sky
      mask or cut from it, and an experienced tester could not find
      the way; the Masks tab wants the next shape's entry under the
      mask it joins, with the mode in view. Asked 2026-10-18

### Speed

On a fanless laptop the fit view follows a slider without a wait,
the sharpen included, and the first Subject mask is there in ten
seconds.

- [ ] A proxy develop for the fit view, as a placeholder and not a
      mode: 2x2 Bayer quads binned to one RGB pixel, shown only while
      the full develop is pending and replaced by it when it lands, so
      the fit view at rest is still the export (§90, §115). The
      camera's JPEG now stands in for the switch (§134), for about
      7 s at 102 MP and 1 s at 24 MP on an M5 (§479): showing it is a
      judgment on those. §270 builds the reduced develop itself, for
      the edited thumbnails
- [ ] The first open of a 45 MP frame: the demosaic (1.2 s) is most
      of a fresh base's 1.9 s on eight threads, now the lens is on the
      GPU (§378, §386); on an M5, 2.8 s of 6.5 to 7.0 s at 102 MP (§479)
- [ ] A Clarity or Dehaze move at 102 MP takes 2.2 s and 2.0 s on an
      M5, nearly all of it the capture sharpen's 20 iterations run
      again after the move (§479, §480). Sharpen at the viewport's
      scale while a slider is held, or only what is on screen
- [ ] The Subject model back on Core ML: explicit zero pads get it
      through the parser, but the Neural Engine's compile ran past
      25 min and the GPU alone is four times slower than WebGPU
      (§477); waits on a compile that finishes in seconds
- [ ] The generative remove on Core ML: on WebGPU over Metal a UNet
      call is 2.5 s on an M5 and 8.7 s on an M1, so Best is 71 s and
      230 s warm, Fast 18 s and 70 s, and the first load 17 s and
      47 s (pixls, 2026-10-28). Waits on the UNet and VAE exported
      for the ML Program compiler at fp16; the Subject Conv's parse
      fix carries over, its Neural Engine compile time may too (§477)
- [ ] The fp16 denoisers on WebGPU, measured 2026-11-01 on an RTX
      5070 Ti with tools/denoise/compare.py (§508): 1.3 times
      quicker (v20 2.9 to 2.1 s, v19 1.1 to 0.8 s), PSNR 57 to 68 dB,
      99.9th percentile 0.015 to 0.03 stops in every band, three
      times Core ML's difference though still invisible; kept at
      fp32 for now, the user's call. CUDA unmeasured, no toolkit on
      the machine
- [ ] The GPU's memory on an 8 GB Mac: the local contrast's planes
      are made per run on unified memory already, and a Clarity move at
      102 MP peaks at 12.1 GB on a 24 GB M5 (§479). Waits on an 8 GB
      Mac at 45 MP
- [ ] An AVX2-and-FMA path for the wavelet chain's taps behind a
      feature check: about as much again on a desktop, nothing on the
      Mac; the first x86 assumption, declined twice (§128, §164)

### AI masks and fill

A sky or a person's face is masked from a menu, and a removed object
is filled from what stood around it.

Decided (§34): ONNX Runtime through `ort`, CPU floor with CUDA, DirectML
or CoreML when found; models downloaded on first use with their
licenses, never bundled; a `greycard-ai` crate that core never sees.

- [ ] AI masks, after Sky (§175): a learned sky matte for dense canopies and twigs
      against glare, where the color-line matte fails (needs clean
      data we assemble); more hard negatives
      for the gate (sky-blue walls, a lake reflecting sky with none
      above it, a snowfield filling the frame); the fog's own plateaus
      where the labels are the sky's outline, and full-size noise
      wherever no floor applies (§345; the seeds and the clip still
      read the diluted map, seeding from the kept one made the
      outline worse); the sky's outline at the frame's top
      edge (a block of blue above a tree left to Foliage); the bimodal
      WebGPU prior
- [ ] Landscape masks, after the first four (§335): Mountain cut by
      the found sky at a ridge as Foliage is (the sum dips to half over
      ten raster pixels at a peak); Grass and Rock next, each a group
      the schema already names, after their hard negatives (a wheat
      field, bokeh, a rockface's plants, a concrete ledge), Ground and
      Road after; Cloud, Cliff, Hedge and a field crop by SAM 3 phrase
      (§252), which the labels cannot name; a cap of one group's matte
      by its neighbors' labels; spires the labels' step under-covers;
      more hard negatives for every bar (fog, snowfields, beaches,
      interiors with plants)
- [ ] The CPU-only ONNX Runtime (§358) run on a processor without
      AVX2, under QEMU user mode with a pre-AVX processor model or on
      a real machine from before 2013; built and reviewed on an AVX2
      machine only (issue #23)
- [ ] MediaPipe's iris landmarks as the CPU fallback for the Iris
      part, where the crop cascade takes about 15 s (§252)
- [ ] Describe your own mask: a free-text field beside the presets,
      fetching SAM 3's text encoder on first use (1.4 GB at fp32, its
      fp16 untried), the phrase and its vector kept in the sidecar,
      and "not found" under the cut (§252: phrases for things absent
      topped out at 0.31). After the parts
- [ ] People: Hair and the other whole-route parts with an edge as fine
      as Whole person's (§279): Subject's matte cut by the part's
      neighbors (face, skin, top) as the kept cells, or a matte of the
      hair alone
- [ ] People: the solid fill Whole person has, for Top, Hair and the
      other whole-picture parts (§263)
- [ ] Face recognition to find the same person across outfits and
      days. Waits on a license search (the strong face-embedding
      models are mostly non-commercial) and on whether a sidecar may
      hold a face embedding, which is biometric data (§253)
- [ ] The fill's level match past §447 and §448: the picture's own
      texture read past the window's 32 px margin so a pattern with a
      30 to 45 px period cannot move a correct fill; a contact shadow
      wider than the 8 px gap, which leaks; the middle's quadratic
      fitted with one octant left out and shrunk by that octant's
      error, so a sun's glow deep in a wide hole cannot move the fill
      2 percent at a spot; a shadow wider than the carry and shallower
      than the fit, still lifted 1 percent
- [ ] The fill's grain estimate capped by the develop's mosaic noise
      model, built on the branch fill-grain-cap and parked (§398): it
      bound on no frame tried, and its flat tile reads silent on a
      clean bright frame. Waits on a fallback to no cap on a silent
      tile, the DCP map modeled, the learned denoiser's leftover
      measured across ISOs, and a frame where it binds
- [ ] A removal harness: 20 to 30 cases through Lightroom's Generative
      Remove and through us, blind pairs, scored (§380)
- [ ] The Fast fill's files uploaded to greycard-rorem-fast (the
      registry already names them, §429), then its time on an M5,
      where Best is 70 s a fill; and whether a choice made on a
      selected patch should also set the default for the next
- [ ] A removal harness run with Fast beside Best, and a longer
      low-rate tail or a hole-mean term in the distillation's loss if
      the shards on large holes are worth chasing (§429)
- [ ] ONNX Runtime's WebGPU Conv is wrong for input channel counts
      that are a multiple of 3 but not 12 (§399); report upstream with
      the reproducer under target/rorem/ortbug

### Color

A camera's own profile and look are a picker away, and a chart shot
makes a new one. This and the library can swap order on what testers
ask for first.

- [ ] The rest of a phone's DNG (§474, §513): the file's
      `BaselineExposure` applied as our neutral plus its value, once
      the neutral is measured from one Adobe-converted DNG of a
      reference body; the 3 MB ProfileGainTableMap, the phone's local
      tone mapping, most of the gap left to its own picture;
      `WarpFisheye` and `FixVignetteRadial`. Test set: three Pixel 9
      DNGs
- [ ] A DNG's warp on the GPU (§513): 0.4 s a develop at four threads
      on the CPU today, once per base
- [ ] A margin past the default crop demosaiced for a DNG's warp
      (§513): the warp reads the crop alone, so where its source falls
      outside (about 2 px at a Pixel wide's corners) the edge repeats
      as a faint smear; Adobe reads the active area's border

- [ ] Color accuracy against a chart: a 48-patch chart shot per camera in
      daylight and under tungsten, kept with the samples, and an ignored
      test that develops each at the default edit and prints the mean and
      worst ΔE per patch, the greys' neutrality and the skin row; the
      first absolute color number, before any matrix work. Waits on the
      chart arriving (ordered 2026-10-13)
- [ ] AgX made the default display curve, after the camera match below
      is refitted under it and §141's baseline and §143's medians are
      re-measured under it: sidecars written before the flip read as
      per channel (a schema rule, so no edited picture moves), a
      picture with no sidecar gets AgX; the viewport timed on an
      integrated GPU and lavapipe first (§231). The match under AgX is
      10 to 20 percent worse than under per channel on the same frames,
      all of it per-frame lightness, and better than today's per channel
      once the exposure match converges (§238), which it does since §240.
      Waits on a refit of the library under it
- [ ] A per-frame contrast term in the camera match's fit, measured
      first with `--match-compare`: the offset and slope oracle puts the
      held-out ceiling at 0.0131 per channel and 0.0146 under AgX (§238)
- [ ] The camera match's black cut on the camera's side only, or scaled
      to the curve, so a table fitted under AgX sees the toe it crushes:
      a quarter of the deep-shadow blocks per channel keeps are lost
      under AgX (§238)
- [ ] The comparison tool's baseline from one pipeline: render per
      channel from the same linear develop the OpenColorIO renders
      start from, so a difference is the transform's and not the
      export's lens profile, crop or look (§227, §230)
- [ ] The scene-referred tail's remaining parts, after the AgX port
      (§226 for the design): (2) the master point curve on a norm with
      the channels scaled by one ratio, the R, G and B curves per
      channel; (3) a gamut compression at constant hue after the curves
      and the look table, before the output matrix, in place of the
      clamps at the curve, the look table and the output, measured as
      §226 measures the hue turn in sRGB; (4) the clamp before the
      curves dropped once the curve's range is guaranteed, a final clamp
      to the encodable range kept. Each behind the comparison over the
      archive sets and its own notes section. Waits on the AgX port
- [ ] The demosaic's direction choices flip on inputs a millionth
      apart (AMaZE, RCD and VNG4 alike), so a frame's GPU and CPU
      chromatic aberration outputs, equal to 1e-5, still export a few
      single pixels dozens of levels apart, which the capture sharpen
      doubles; decide whether a tie-break makes them stable (§264)
- [ ] The color curves' lightness axis near black: the shift is looked
      up by Oklab lightness, a cube root whose slope is unbounded at
      zero, so a few parts in a billion of light at a clipped channel
      read as a lightness near 1e-3 and a curve with a point under 0.05
      turns them into a cast of whole levels; the random parity test
      draws its points from 0.05 for that reason. Decide the axis's
      floor or shape so the curves are stable at black, and lift the
      draw back to the panel's 0.01. §239's fade at black (a shift reads
      light under 2e-6 as black) may be that floor; try the 0.01 draw
      over the seed sweep
- [ ] Camera match follow-ups (§181): the error held out over every
      frame (§240) deciding whether a new fit of the same body from
      another folder replaces the table there, where today more frames
      wins; the per-lens radial report fed
      into the vignetting line below; `style:` in the filter text;
      Sony's Picture Profiles and Panasonic's newer Photo Styles (§265)
- [ ] The survey's ends for NEF, ARW, RW2 and DNG (§324): the IFD
      chain, EXIF and maker note walked to where they end and read as
      the head, the DNG's directory at its end as the tail, so those
      bodies read kilobytes over a share where today they read whole;
      a size cut through a TIFF is never safe, since a zeroed gap can
      make rawler ask for gigabytes, which aborts and no guard catches
- [ ] Camera match Refine: the frame on screen, or the selection,
      pinned into a group's sample and the group fitted again, with the
      pinned frames' error and the rest's said before and after; a
      pinned frame that will not register, or has an adaptive setting
      on, refused with the reason. Fast because each frame's block
      pairs are cached under the cache directory, keyed by the frame's
      content hash and a version of the default develop, so a fit over
      a group develops only the frames it has not seen; the held-out
      error over every frame (above) reads the same cache with one
      frame left out each time, and its per-frame list is what says
      which frames to refine with. Never a nudge of the table toward
      one frame: a refined table is still one fit over a sample, so
      the replacement rule still counts frames
- [ ] Profile making from a chart shot, after dcamprof (§78)
- [ ] A DCP's look table applied with its matrix and hue map, as
      RawTherapee and ART do, so Adobe's Camera Standard, Landscape,
      and Portrait differ from each other (issue #49, 0.8.1; §122
      applies the matrix and hue map only); the tone curve stays a
      Look choice. After 0.9.0
- [ ] An Adobe lens profile's off-center optic and its green-channel
      and tangential chromatic terms, unmapped in §497 (a center field
      in core's LensCorrection first)
- [ ] A status line and a cancel while Stack bursts reads a big
      root's previews, a minute a card and longer for Fuji
- [ ] RawTherapee's DCP profiles offered for a fetch: they are made by
      that project under the GPL, so they can be hosted by us as the
      Sky model is and fetched into the profile directory from the
      Camera section for the bodies they cover, a one-click profile on
      Linux before the chart shot exists (§78); with attribution and
      their license shown as lensfun's is. Waits on a tester asking,
      since it covers a fraction of bodies and the chart shot is the
      real answer
- [ ] The monitor's own profile on Windows: read it from WCS, so
      "System" means the display rather than sRGB, as colord does on
      Linux and ColorSync on a Mac (§428). Today the viewport falls
      back to sRGB there unless an ICC file is picked by hand, which a
      wide-gamut monitor makes wrong (§137)

### The library

Directories stay the truth; the catalog is a rebuildable index (§72).
A moved shoot is found, not flagged missing, and a Lightroom catalog
comes across with its ratings and collections.

- [ ] The filter bar's last two chips: a "none" chip for frames with
      no value for a facet (decide it with a library of phone JPEGs in
      hand), and an error row in the index for a file whose hash fails
      so it stops showing under every chip (§168). The all-roots view
      and the filter remembered between sessions are there since §174
- [ ] A frame renamed on one side of a backup pair keeps its old name
      in the destination's stack copy (§511): a stack's revision
      without file names, or Back up renaming the destination's file;
      the user's call
- [ ] The Lightroom mapper, after §342: the scales the fit proposes and
      did not ship (highlights, blacks, the curve, the mixer's bands)
      wait on a second shoot with export history; six exports from
      the catalog of about 280 (the user's, on Windows): a virtual
      copy of one GFX frame and one R5 Mark II frame with every
      develop setting reset, then each with Shadows +50 alone, then
      Highlights -50 alone, into the shoot's export folder so the
      history pairs them, to tell the profile curve's shadow lift
      from the sliders' reach (both bodies sit a stop under Lightroom
      below L 0.3); the mixer's green band against Lightroom's; Adobe's
      `BaselineExposure` from a DNG per body as the generalization
      check; whether an unedited frame of a body takes its offset on
      import (today only edited frames do, so neighbors land 0.9
      stops apart); the refitted Provia, Reala Ace and R5 Mark II
      tables proposed by the fit darken against the no-look render,
      a question for the camera match before they are installed;
      the grading wheels' scale, shipped at two thirds by eye
      (§365), from exports with only the highlights or the shadows
      wheel set, and the lightness curve against Lightroom's
      Blending and per-wheel Luminance, which the mapper does not
      read
- [ ] Re-map frames untouched since their import: an imported frame's
      history starts at the "Imported from Lightroom" state, so a
      later mapper (a fitted scale, a key newly carried) can be run
      again over every frame whose history holds nothing after it,
      with no edit of the user's moved; the mark the import leaves is
      what makes it possible, so it stays reliable
- [ ] darktable's XMP history imported as RawTherapee's profiles are
      (§328, §340), if a tester asks
- [ ] The RawTherapee sheet: a menu entry back into it for a folder
      already answered, and its frame list laid out lazily rather than
      capped at 200 (§340)
- [ ] A time zone shift beside the capture time shift: a "Time zone"
      choice in the sheet that moves the local time and the EXIF
      offset (OffsetTimeOriginal, OffsetTimeDigitized, the XMP date's
      zone) together, so the moment stays the same and a camera left
      on home time reads local; the plain shift keeps the offset as
      the camera wrote it, right for clock drift and wrong by the
      shift for a zone, which readers that combine the two will show
      (decided 2026-10-18, kept simple for 0.8.0)
- [ ] Lightroom catalog import follow-ups (§337): a catalog row for a
      camera's JPEG beside its raw mapped onto the raw's frame (wants a
      catalog made with JPEGs as separate photos; none held has one); a
      Flip and the Upright transforms carried, not counted; a camera
      rule passing some cameras carried once the filter has a union of
      terms; a root guessed from a folder holding part of a shoot
- [ ] A map: the frames' EXIF GPS on tiles, a click to select them, a
      position given to a frame by hand, and a position copied from a
      frame that has one to frames that do not. Low priority; asked
      again 2026-10-19 (issue 30)

### Culling and search

A card's worth of frames comes back in a first-pass order: bursts
grouped with the sharpest on top, missed focus and closed eyes said,
and a picture found across the library by a phrase or by a person.
A model suggests; a flag, a rating or a keyword is only ever set by
the user.

- [ ] A focus score for every frame from its embedded preview, on the
      subject where the Subject mask finds one and on the frame
      otherwise; a sort, a `focus:` term, and the order inside a burst
- [ ] Closed eyes said from the face parts the Parts mask already
      finds (§252), as a term and a mark on the tile, never a reject
- [ ] Focus peaking in the lightbox: its tiles are Slint images, not
      the viewport's draw, so Shift+J does not reach them (§542)
- [ ] Search by phrase: an image embedding from a CLIP-style model
      (SigLIP 2 a candidate, its license and training data checked)
      computed by the indexer from each preview and kept in the
      library index, never a sidecar; a phrase in the filter field
      ranked against them
- [ ] Suggested keywords from the same embeddings, kept apart from the
      user's own until accepted into the meta section
- [ ] People: faces found and grouped by a clean-license model, a group
      named once, a Person facet in the filter bar and a `person:`
      term, the names written as keywords so other tools see them.
      Waits on the face-recognition license search (AuraFace's
      recognizer and SFace the candidates, their training data
      unasked) and on whether the index may hold a face embedding,
      which is biometric data; never a sidecar (§253)

### Merge

A bracket, a focus stack, or a panorama becomes a linear DNG beside
its sources, through one pipeline (§523): each frame to camera RGB,
registered onto a reference, put on one scale, merged by weights,
written as a float DNG with its headroom and a noise profile. The
brackets and the panorama to shoot for testing are in
`docs/test-frames.md`.

- [ ] Next: the re-merge's rename over an open file tried on Windows
      (§541; a bounded retry in `promote` if it fails there, §543)
- [ ] Phase 1's leftovers (§525): the CA fit taken from the frame with
      the most voting tiles rather than the reference, measured on a
      real bracket with a clipped sky (§529); the Highlights pull's aim
      at display white measured on a real bracket (§528); a noise model
      with the merge's steps in it for its readers; a faster Deflate in
      rawler, upstream (libflate does 30 MB/s a thread, §536); the
      per-frame learned denoiser offered in the action and the command
      line; the measurement on a real tripod bracket
- [ ] Phase 2's leftovers (§530): a merge in strips so the 100 MP cap
      can go (a 20-frame handheld set is refused only after its walk);
      a sphere; the uncropped canvas; the learned denoise on by default
      once the store offers the tier; the lock check against a merge
      started in the same microseconds
- [ ] Phase 3, the handheld bracket (§523): the similarity fit, a
      better resampling kernel in `stack.rs` and `register.rs` (the
      Engine pool's warps item, pulled forward), and reference-frame
      deghosting, on by default
- [ ] Focus through the Merge action and `greycard merge focus`, after
      phase 1 (§125, §523)
- [ ] A bracket finder: a burst whose EXIF exposures step marked as a
      bracket, so the merge can be asked of a whole card (§523)
- [ ] A center for the vignette: two sliders or a drag on the picture
      to move the shape off the frame's middle, as darktable has and
      Lightroom has not; the center kept as fractions of the frame so
      it sits on the crop
- [ ] A Compress slider in the Light section: the guide plane of §85
      pulled toward mid grey by a strength, so bright regions come down
      and dark ones up together and the texture inside each rides on
      top untouched (Durand and Dorsey's base and detail, with the
      guided plane as the base; no new pass, the plane is cached
      already); an Auto from the frame's range in stops. One slider,
      not a filmic module with a shoulder and a toe. Wanted most for a
      merged bracket written with headroom; waits on the guided plane's
      halo measurement in the feel track, since one slider moving the whole
      plane makes any halo count

### The engine

The command line renders an edit exactly as the editor exports it,
through one develop engine below the window. An outside review of
the layering (2026-10-29) found the direction clean and the engine
still inside the window's crate; the big move waits on its design
section, and the small items go first since they shrink what it
touches.

- [ ] The editor's `--export` flag dropped, one release after §514
      made it an alias of `greycard export`

---

## Pool

Not yet in a track or in Next. An item joins one when its "waits on"
clears or a tester asks for it.


#### Editor

- [ ] Failures as a dialog queue, not a status line: a failed outcome
      today becomes "failed: {message}" in the status bar
      (deliver.rs); a queue of typed errors with a kind, the frame it
      concerns, and a next step (retry, open the log, report), shown
      one at a time and never lost under the next status. A sidecar
      that could not be written gets its own recovery sheet (an
      outside review against CalibRaw, 2026-11-02)
- [ ] A Flatpak on Flathub: the AppImage's contents under the
      freedesktop runtime, the GPU, the roots and the model cache as
      portals or holes, the ort dylib and the models fetched into the
      sandbox's cache; the one install path Linux testers ask for
      first, and where the other open editors are found (an outside
      review, 2026-11-02)
- [ ] Translations: the window's strings through Slint's `@tr` and a
      gettext catalog, English the source, the first languages the
      ones testers offer; the guide and the release notes stay English
      (an outside review, 2026-11-02)
- [ ] Every section switch and button a bindable action in the keys
      table (§390), so a key applies the lens correction or opens the
      crop; a key that applies a named preset beside it
- [ ] Section order and default folds: drag sections into the order
      wanted (the pipeline's order unchanged), fold a set by default;
      and the scopes folded away on a small screen, their space to
      the panel
- [ ] Before and after: the picture against a chosen history step or
      snapshot under a wipe the pointer drags; and the last render
      kept when a section is switched off or on, so Denoise's toggle
      shows its difference at once under the wipe instead of after
      the whole chain runs again (pixls, 2026-11-01)
- [ ] Bezier handles on a curve's points, so a point's reach along
      the curve can be set
- [ ] The crop's grid: its lines' count and spacing chosen, so a line
      can sit near a horizon
- [ ] History grouped by tool: a run of steps on one tool folded into
      one line that opens, so the timeline reads clean and nothing is
      lost
- [ ] Spektrafilm: Andrea Volpato's spectral film simulation, a
      physical model of exposure, development, printing, and scanning
      from measured emulsion and paper data rather than a LUT, with
      grain, halation, and diffusion as things that happen on the way;
      darktable has a module in progress derived from it. Ours would
      be a look of a new kind beside the LUTs, with the data pack
      fetched like a model. Waits on a read of the code's and the
      data's licenses, and on a port decision: GPU, and at what cost
      per frame. Asked 2026-10-18
- [ ] The work-area fit (§500) tried on the Mac with a large and a
      side Dock and on Windows with a task bar at the top and two
      monitors of different scale; built from the sources, not run
- [ ] The long `run` and `develop_job` in `worker/` and `Look::run` in
      `roots/` cut into functions, a later round after the splits
      (§316, §318, §321)
- [ ] The window's structure (0.8.0), in mechanical passes each
      landing green, each alone with no other window branch in
      flight. Landed: the CLI's `main.rs` split one file a subcommand
      (§341); the window's `State` grouped by owner (§344: views,
      develop, culling, sheets, work, window, launch; 17 fields left
      at the top). Measured 2026-10-18 before carving the library
      views: one rustc compiles the crate's 163,000 hand-written
      lines with 392,000 generated from 17,400 lines of `.slint`,
      six times the hand-written code by bytes, at 96 to 106 s and
      10.6 GB for rustc from clean (`-j 4`); the library side is
      44,000 lines, about 7 s of that, and 257 of its 419 tests build
      the whole window, so a views crate alone halves nothing, and
      it cannot compile its own `.slint` (one component tree a crate,
      the grid and strip children of `App`). Tried in a workspace
      copy, the Slint UI in a crate below the window took the
      window's build to 28 s and 2.4 GB, its test build from 120 s
      to 40 s, an incremental rebuild from 28 s to 13 s, with a
      three-line change to the window and every `app.set_*` call
      kept; the Slint crate's 74 s and 9.6 GB are paid only when a
      `.slint` file changes (`cargo build -p` on it alone picks a
      different feature set and forces a rebuild; build through the
      window). So the order is: the Slint UI into its own crate
      (landed, §351: window build 24 s and 2.3 GB, incremental 8 s;
      `cargo build -p greycard-slint` alone still compiles a wasted
      second copy, closed only by a workspace-hack list, left open);
      the desktop file chooser out of `export.rs` into a window
      `chooser.rs` (landed, §356); a render crate
      between edit and the window holding the finish, agx, detail,
      export, border, watermark and geometry with their test helpers
      (landed, §361: 104 tests moved, the window's build within
      noise of before, so the gain is the boundary and not the
      build; the CLI and the headless export still reach it through
      the window and could depend on it directly); a thin views
      crate with only the shared types
      (`versions::{Key, Cell}`, `rows::FromRow`, `file_name`, called
      from 51 places outside), depending on the Slint and render
      crates and never the window; the model modules with no outward
      edge (filter, sort, grid, selection, tags, mounts, placeholder:
      3,800 lines, 60 tests); then those with one seam each
      (collections, culling's prefetcher, previews, thumbpool and
      `worker/thumbs.rs` behind a thumbnails handle the views crate
      defines and the worker implements: 6,900 lines, 68 tests);
      measured again. The bindings (browser, culling, roots, tree,
      library, lightbox, rows, cells, the small panels) wait for
      0.9.0 behind three seams: opening a frame (`open_frame`,
      `open_row`, `pick_pending` reach two dozen develop functions
      and 70 develop fields; a small Develop trait, the mosaic
      tracking moved to the develop side), culling drawing through
      the develop's viewport (`st.develop.zoom` 15 times, the
      renderer, `lut_for`, `render::Tile`; the loupe and compare
      drawing stay in the window), and the thumbnails on the worker
      handle. The crate tree otherwise stays as it is: core knows
      nothing, edit and library sit on it, the window on everything
- [ ] A cancel token through the develop: the worker drops a stale
      result by generation but never interrupts the develop making
      it, so a big frame or an export batch runs to the end after
      the user has moved on. Core's develop takes a stop flag checked
      between stages, the worker sets it when a newer request
      supersedes, and the export queue's cancel sets it too. An
      outside review asked 2026-10-19
- [ ] A size report in CI: the files and functions over a line count,
      printed on every run and compared with master's, as a report
      and not a gate, so growth is seen when it happens; the five
      largest files are 4,700 to 6,500 lines (cull, index, edit's
      lib, the catalog reader, browser) and the first to split are
      startup and mask
- [ ] A missing camera profile said where the user looks: the
      status line, the export's report of what the edit went without
      (with the headless export's exit code 2 as for a missing
      look), and a preset or a sync that would spread the missing
      name; today only the Camera section says it, and only while
      open (§373)
- [ ] A sidecar batch's writes a few at a time (four or eight) where
      the share rewards it, measured on a network share and on a Mac
      first (§498)
- [ ] A rename into place retried on Windows' sharing violation, from
      a scanner or a sync client holding the old sidecar open (§498)
- [ ] The learned mask and fill caches keyed by the develop they were
      made from: the window's Export reuses the mask its viewport made
      from a base whose CA ran on the GPU, where `--export` makes it
      from the export's CPU base (AE 174 on a Subject frame), and the
      disk cache of masks is keyed by file, model and shape, so the
      first run to make one decides it for every later edit (§236)
- [ ] The panel's look, refined as the editor grows (§14); in develop
      order on Develop, Crop, Masks and Retouch tabs since 2026-09-14
- [ ] Choose output color space/gamut for export: sRGB, Display P3 and
      Rec.2020 since §20; the viewport shows the chosen one since §42;
      ProPhoto as an output (a pixls tester, 2026-10-22)
  - ProPhoto as a working space: no (§84). It stays an output space and
    an intermediate inside the operations defined in it
- [ ] The develop sections in an order of the user's: a list of
      section names in settings, the panel a repeater over it with each
      slot instantiating the section its name says, the Masks tab
      following the same order, Look and White balance pinned at the
      top and Monitor, Proof and Demosaic at the bottom, and up and down
      buttons for it in the Settings sheet. Goes with the develop
      panel's move out of app.slint, since that is the same rewiring
- [ ] Resizable side panes: drag the left pane's and the develop
      panel's inner edge, today's widths the minimum, about 500 px the
      most, each kept in the settings; first see what a viewport
      resize costs per pointer move, and hold the redraw to the drop
      if it is a develop (hiding landed, §178; the strip stays fixed)
- [ ] A software GPU when there is none: the worker and greycard-gpu
      ask wgpu for a high-performance adapter and stop when there is
      no adapter at all (a headless box, a VM, a remote desktop), so
      ask again with `force_fallback_adapter` and take lavapipe or
      WARP, slow but right; the develop already runs on the CPU, the
      viewport does not. A tester on such a machine decides whether
      the viewport wants a CPU path too
- [ ] A scene mode for the R, G and B curves: the color curves on
      log-encoded scene values before the display curve, in stops about
      mid grey, as a grading tool (what Resolve's log controls do),
      beside the display-referred curves and not in their place; waits
      on the scene-referred tail above, which decides where the curve
      sits
- [ ] The export destination from tokens, with the date and the
      frame's folder, so `~/Raw/2026/2026-10-22_shoot` exports to its
      twin under `~/Edited` (a pixls tester, 2026-10-22; the name
      pattern landed, §494)
- [ ] A list of marks so text and a logo go on one export; a plate
      or shadow behind text; a bundled font so exports match across
      machines (§157)
- [ ] Geometry in a sync, mapped through each frame's aspect and turn
      as masks already are (§156)
- [ ] The range masks' sample from before the Detail section, so dehaze
      stops moving a luminance window; needs a second full-size texture
      on the GPU (§158)
- [ ] The 5x5 color mean at a fixed scale, so the viewport and a
      smaller export agree on a color window's edge and the mixer's
      hue (§158)
- [ ] A dropper for the luminance window's edges and a swatch of the
      color window beside its sliders (§158)
- [ ] The vibrance protection's skin window (§60, 55 degrees plus or
      minus 15) against the 10 to 59 degrees a pale face measured
      (§158): widen it or not; a change to existing edits
- [ ] A proper export queue (§213): a Destination row on the export
      sheet (the folder, Change..., the last one remembered) that Export
      and Add to queue both take, so queuing opens no chooser; and the
      queue as a list, each set a line (frames, preset, folder, when
      queued) that can be removed, reordered or sent elsewhere, with
      Export the queue and Clear; the queued frames marked in the strip
      and the grid. Covers the destination half of the naming-patterns
      line (§157) and the last-location line above


#### Library

- [ ] Geotagging from a GPX track: the camera's UTC offset, a manual
      offset for a clock left on the wrong time, a matching tolerance,
      the position written to the sidecar and the XMP, as digiKam and
      darktable do

- [ ] Folders opened outside every root looked at again as a root's
      folders are: opening a folder indexes it, but nothing passes over
      it afterwards, so when it is deleted its rows are never marked
      missing and stay in the index for good (§160's rule covers only
      what a pass reaches); a light look at those folders on the
      launch pass or the network poll, a gone folder's rows marked
      missing
- [ ] "Forget missing frames" on the Settings sheet, with how many and
      the oldest's date, so the missing rows go without the CLI's
      `--prune` (§160)

#### Engine

- [ ] The window's horizon call (`greycard_core::horizon::horizon` in
      panel/crop.rs) moved behind the engine; a core op outside
      develop, so the layering test (§540) cannot see it
- [ ] The unwraps counted down: about 230 `unwrap` and 380 `expect`
      outside tests, most of them lock `expect`s in the engine and
      the rest in the window and the models; a lock helper that
      recovers a poisoned mutex, `?` with context where a Result
      already flows, and the count printed by the size report so it
      only falls (an outside review, 2026-11-02)
- [ ] The 52 `too_many_arguments` allows looked at one by one, a
      struct where the arguments are one thing (an outside review,
      2026-11-02)
- [ ] The sidecar written at the oldest schema that holds the edit,
      so an older build reads a newer build's file until a field it
      lacks is used; today a newer version is refused outright
      (migration.rs). Each migration step carries the release it can
      be removed in, checked by a test (an outside review, 2026-11-02)
- [ ] `greycard develop` through the stage list: the CLI's develop
      calls core ops in its own order (architecture.md), the one path
      outside `walk`; a `Purpose` for it, so there is one pipeline
      (an outside review, 2026-11-02)
- [ ] Export on the GPU, or the reason it is not written down: every
      export runs the CPU finish while the viewport runs the shader
      twin, so a fast card never speeds an export; the twin is held
      to the CPU by the parity test, so the shader could serve export
      at full size through tiles, with the CPU as the fallback and the
      reference (an outside review, 2026-11-02)
- [ ] Samples below black kept through the demosaic: measured (§549),
      the clamp lifts the darkest 1% by 3% at ten stops under white
      on an ISO 1250 frame, under one raw unit, and dropping it would
      rework four demosaics, the CA passes, and the denoise's
      transform. Waits on an ISO 12800 frame with true black in it;
      `examples/below_black.rs` takes the table again
- [ ] The profiled denoise's unbiased inverse has a pole below zero
      when the fitted `b` is zero (`Vst`, §549): nothing reaches it
      today, since the develop never hands the denoise a negative, but
      a guard belongs there
- [ ] `stack.rs` and `register.rs` warps: rows written the way the lens
      correction's were, with captured models re-read per pixel; the
      lens gain (§165) says they are the next place to look
- [ ] The X-Trans CA correction's shortfall against Bayer's (86% and
      98% of the error left where Bayer leaves 80% and 90%, §343): the
      green the resample reads is the limit, a demosaic's green gains
      0.4-0.9 dB in experiments; a sharper green that uses the red and
      blue around each site as Markesteijn's first step does, at a
      cost the correction can carry, then the vote again (the
      stand-in's 0.77 under-read; a gain and votes on the sites were
      both measured and not kept); a GPU version if the CPU's 0.65 s
      becomes what a develop waits on
- [ ] The noise estimate on a frame of texture with no flat area, on
      any mosaic (§331)
- [ ] The capture sharpen's radius measured from an X-Trans mosaic
      (§322)
- [ ] A learned denoiser for X-Trans, in this order (§322). First the
      contract: a packing of the 6x6 pattern into the network's input,
      since the Bayer four planes at half resolution have no
      equivalent, and that decides the data's shape. Then the data,
      from three sources: the bulk from the archive's low-ISO Bayer
      frames, demosaiced and re-mosaiced onto the X-Trans pattern
      (thousands of frames, license ours); raw.pixls.us's base-ISO
      RAFs, CC0, one to a few per body across X-Trans I to the
      current sensors in both compressions, as real-pattern training
      examples and as the held-out validation set, so the reported
      number is on real X-Trans; and the noise model per body fitted
      from the flat regions of raw.pixls.us's high-ISO RAFs and our
      own Fuji samples. Our side writes the re-mosaic export and a
      fetch script for the pixls set; the training is the user's, as
      the Bayer tiers were. The README's training clause grows the
      CC0 set
- [ ] A GPU X-Trans demosaic, if the CPU's 0.6 s on 40 MP becomes what
      a develop waits on (§322)
- [ ] A sync's first read of frames the library has not indexed yet
      runs on the window's thread, once a frame a session (§322)
- [ ] Per-camera hot pixel defect map. Waits on a place for per-camera
      data and a way to take dark frames in (§13s)
- [ ] A per-photo switch for the hot pixel repair in the edit schema,
      since a misfire (a colored point light one site across on busy
      texture) has no escape in the editor; only the CLI turns it off
      (§229)
- [ ] The hot pixel others test counts a neighbor as lit by ratio and
      sigma alone, so a defect's own bleed into the next photosite
      (about three percent of its excess) spares it; ask the excess to
      be a fraction of the candidate's, measured on the lanterns' one
      recurring site (§229)
- [ ] The sharpen's corner radius offset. Waits on a per-lens notion (§21)
- [ ] Segment highlights' rebuild modes. Waits on a frame that shows the
      need (§13o)
- [ ] Exact denoiser tiling, only if a memory budget demands it (§13p)
- [ ] The 5x5 patch at high ISO, only if a print prefers it (§13v)
- [ ] A tool for a broad out-of-focus color cast: §61's lavender bokeh
      discs, which the defringe cannot reach because the cast is wider
      than any local mean it takes (§74)
- [ ] Nikon High Efficiency, after the build feature (§266): Adobe
      DNG Converter's decode of the eleven frames against ours, which
      settles the curve above 1000 DN and the 3% magenta midtones;
      the report on dnglab/dnglab#835; the decoder read against the
      standard (§176); HE* from a Z8 or Z9. Waits on a Windows or Mac
      machine for the converter
- [ ] Nikon High Efficiency in the published builds: the JPEG XS
      patent pool charges per copy and exempts nothing (§176); the
      macOS system decoder is the one way around it

#### Immich

- [ ] Check that Immich reads greycard's XMPs from an external library:
      which fields land (keywords above all) and whether it writes back
      into the sidecar; an Immich in a container over a copied shoot
      (§195)
- [ ] A post-export command in an export preset, run with the exported
      paths, so `immich upload` (or any other tool) takes the developed
      picture; the destination folder for a watched library is the
      pool's naming-and-destination item. Waits on the XMP check (§195)
- [ ] Edit in, the rest of it (§499): a list of editors found once
      rather than one chosen app; a raw-first tool (PureRAW, Photo AI)
      given the raw and its DNG offered in place; the edited TIFF's
      watch kept while the file is listed rather than an hour. Try
      the Mac and Windows launches on the machines first
- [ ] A plugin protocol, after 1.0 and only on demand: an executable
      given a rendered file or a mask over JSON on stdin and stdout, a
      process boundary rather than in-process code, so a plugin's
      license and crashes stay its own
- [ ] A paragraph in `where-greycard-fits.md` on greycard as the raw
      developer for an Immich library. Waits on the XMP check (§195)

#### Nice-to-have

- [ ] Flexible dual monitor support; color management per display.
- [ ] Different frameline options when cropping
- [ ] Camera tethering
- [ ] Idea: AI sharpening
- [ ] Idea: AI color grade matching to a reference
- [ ] Idea: AI highlight recovery
- [ ] Maybe: MCP
- [ ] Maybe: Depth mapping for blur

#### User Requests

- [ ] MIDI Controller support
- [ ] Zoom bar for grid and lightbox
- [ ] Spektrafilm, darktable's spectral film simulation
      (measured film and paper data, grain, halation), in place of
      the look stage rather than over it, as darktable advises. Waits
      on darktable's module shipping and on the licenses of
      spektrafilm and its data pack
- [ ] HEIC/HEIF as pictures: HEVC is patent-encumbered, so nothing of
      ours ships a decoder; macOS through ImageIO, Linux and Windows
      through a libheif found at runtime and loaded by name as ONNX
      Runtime is, a file skipped with a log line where none is; AVIF
      beside it from a pure-Rust decoder
- [ ] A toolbar of common actions, the set the user's, with the
      menu bar (pixls, 2026-10-22)
- [ ] Clear the card after an import, behind a confirmation and only
      once every copy's hash has been checked (pixls, 2026-10-19)
- [ ] The toolchain that rendered an export embedded in its metadata,
      so opening the export can find the source and reapply the edit
      (pixls, 2026-10-19)
- [ ] The picture zoomed in steps from the keyboard, as the grid's
      tiles are with plus and minus (pixls, 2026-10-22)
- [ ] Saturation in the shadows or the highlights alone, with the
      hue held (pixls, 2026-10-24)
- [ ] Structure's strength judged on real frames (§507): +100 reads
      as about Clarity +35; the gain is one constant (1.5; the review
      suggests 2.5 to 3, or epsilon 0.1 with 2)
- [ ] Color noise reduced on its own, the luminance noise left
      (pixls, 2026-10-24)
- [ ] A mask's adjustments nested under the mask, so a folded mask
      is one row (pixls, 2026-10-24)
- [ ] A default preset applied to every frame on its first open, so
      the tools a user never wants start off (pixls, 2026-10-24;
      asked again as a saved baseline, discussion 59, 2026-10-30)
- [ ] The brush's Flow is what other apps call Opacity: one stroke
      lays down at most its amount, and strokes build on each other.
      Decide whether to rename it or add a Flow that builds within a
      stroke (pixls, 2026-10-25)
- [ ] ART's `.arp` profiles read beside a raw as RawTherapee's
      `.pp3` are: the same key file, found by its extension
      (pixls, 2026-10-24)
- [ ] Constrained mask drags: Shift holds a linear mask to the axes
      or 15-degree steps and a radial mask to a circle
      (pixls, 2026-10-24)
- [ ] The clipping warnings as a pair of plain buttons, with the
      toolbar (pixls, 2026-10-24)
- [ ] A color negative photographed with a digital camera inverted:
      the film base sampled as the white point, each channel turned
      about it, a curve per film stock later (pixls, 2026-10-28). The
      inversion a stage of its own before the tone tools, so Highlights
      still lightens the highlights of the positive; swapping the
      black and white points, as C1 and Lightroom make one do, turns
      every slider round (reddit, 2026-10-31)
- [ ] The Finder's reopen of a running greycard (a Dock click with
      the app open) brought to the running window; it comes as an
      Apple event winit does not deliver (§504)
- [ ] The download progress shown where it cannot be missed while a
      dialog has the window greyed out (discussion 51, 2026-10-28)
- [ ] A lighter canvas and a white one, for the light theme
      (issue 53, 2026-10-29)
- [ ] A fill from the near surround only, so a logo on a red button
      does not take the blue round it, and a repeated object is not
      drawn again (issue 54, 2026-10-29)
- [ ] A smaller remove brush, and a selection redrawn rather than
      started over (issue 54, 2026-10-29)
- [ ] A mark when a remove's region is filled below full resolution
      (issue 54, 2026-10-29)
- [ ] The status area clear of the picture, or dismissable; the scroll
      bars at their hovered size always; the Export sheet's text crisp
      at 1080p (discussion 51, 2026-10-29)
- [ ] Subject's edge through hair under an extreme correction: the
      matte is made on a 1024 square and brought up to the frame, so
      wisps and locks come out as a soft band with a light fringe
      against a background pushed near black. A second pass at full
      resolution along the edge band, as Sky's twice-size pass does
      for wide frames (discussion 51, 2026-10-29)
- [ ] The rest of the close on a Mac's Quit, which ends the process
      from the event loop: the title's send, an import's stop, the
      worker's stop, and the index's close moved into the loop's
      exiting hook (§449)
- [ ] The library and the caches placed where the user says, off the
      system drive (discussion 51, 2026-10-29; the folders open from
      Settings since §484)
- [ ] The mask section's name field and its Show mask and Invert row
      do not share a left edge; a mask's row deletes with a bin and a
      shape's with an x: one edge, one glyph (discussion 51,
      2026-10-29)
- [ ] Luminance, hue, and chroma ranges in one mask shape with a curve
      for each, and chroma as a range rather than a floor, as ART's
      parametric mask; today it is a Luminance and a Color shape under
      Intersect (discussion 59, 2026-10-30)
- [ ] A pivot for Contrast, so it turns about a chosen grey rather
      than the middle (issue 56, 2026-10-30)
- [ ] Sync from the open frame onto the selection by a middle-click
      on it, as DaVinci does (issue 56, 2026-10-30)
- [ ] Dust and scratches on a film scan found and healed in one go,
      each spot listed so a wrong one can be removed or redrawn; the
      camera-scanning workflow alongside digital raws (reddit,
      2026-10-31)

- [ ] Refine edge at full resolution (§503): the fitted a·Oklab+b
      sent to the finish and the shader and applied per pixel, so the
      edge is finer than the 2048 grid's 4 px at 45 MP and a Sky's
      wide raster keeps its detail; and the filter off the window's
      thread while a slider drags
- [ ] The guide's worked example of each fade (Feather, Flow,
      Luminance, Color), numbers checked against the code
      (discussion 59, 2026-11-01)
- [ ] Drawn shapes as masks: a polygon and a curved path with a
      feathered edge, inverted and combined like the rest
      (discussion 59, 2026-11-01)
- [ ] A Midtones slider in Light between Highlights and Shadows, so
      the middle lifts without the curve (discussion 59, 2026-11-01);
      weighed before and left out as unneeded, not settled
- [ ] The develop's base kept on disk, so a frame left and opened
      again on a small card is quick without its neighbors' bases
      held in memory (discussion 59, 2026-11-04)
- [ ] A mask's edge tools beside Feather and Refine edge: grow and
      contract by a radius, a blur, and the holes filled, each on a
      shape and on the whole mask (discussion 59, 2026-11-04)
- [ ] A fill's area painted over again after the fact: strokes added
      to an existing retouch and the fill made again (discussion 59,
      2026-11-04)
- [ ] A lens profile picked by hand for a file that names no lens:
      the Lens list should already show every LCP in the profile
      folder; check with the tester's CR3 and LCP (discussion 59,
      2026-10-31)

## Blocked

Work that waits on someone outside the repo: a protocol reaching
Slint, a runtime exposing what it knows, a rawler release. It joins a
release when the wait clears.

#### Platform
- [ ] Report a problem fills in the CPU as it fills the OS and the GPU
      (report.rs): `/proc/cpuinfo`'s model name, `sysctl
      machdep.cpu.brand_string`, the registry on Windows. The bug form
      has the field since 0.8.1; two of ten issues turned on it
- [ ] The models' adapter checked for a CPU type as the window's now
      is (§374): ONNX Runtime's WebGPU provider picks its own adapter
      through Dawn, and on a machine whose only Vulkan adapter is
      lavapipe it may run the models as software shaders, minutes a
      frame; the CPU provider is the right one there. Unverified
      either way; a run under VK_DRIVER_FILES naming lavapipe alone
      with a model fetched says which
- [ ] A Windows install for all users: an -AllUsers switch on the
      installer that elevates itself, puts the program under Program
      Files, and writes the common Start Menu folder, the machine's
      Path, the file associations under HKLM, and an entry under the
      machine's Uninstall key, with uninstall.cmd taking the same
      switch; today GREYCARD_PREFIX puts the files there and the rest
      stays the one user's. An installer package (Inno Setup or WiX)
      from the release workflow is the step after, and where signing
      goes. Asked 2026-10-19. Two testers would rather unzip anywhere,
      as RawTherapee ships, than install: the zip runs in place today
      from bin\, and the guide says so; keep it beside the installer
- [ ] A GPU path for the models on Windows on ARM: pyke publishes an
      ARM64 Windows build of ONNX Runtime with DirectML, no WebGPU,
      and DirectML ships with Windows, so ort-dylib could make our
      library from that archive and the models would leave the CPU
      there. Waits on a tester running the aarch64-windows package
      at all (§372), then on reading what the DirectML provider
      needs beside the library
- [ ] A SHA-256 sums file attached to every release, written by the
      release workflow over the four packages, so a download can be
      checked and an updater has something to verify against; a
      signature (minisign or sigstore) after it
- [ ] Self-update from the editor: the release check already finds
      the newest tag; the step after is to download the platform's
      package, verify it against the sums file, install it where the
      install script put the running copy (rename-then-replace on
      Windows, the app bundle and its quarantine flag on macOS, the
      tarball's layout or the AppImage file on Linux), relaunch, and
      roll back if the new copy does not start. Waits on the sums file

- [ ] The window's own monitor for the profile, without choosing it.
      Waits on the Wayland color-management protocol reaching Slint's
      winit backend (§17)
- [ ] HDR output. The same wait; check each Slint release (§5, §17)
- [ ] Wide-gamut output: a profile away once the compositor says which
      monitor (§17)
- [ ] Self-updating: when the check (§186) finds a newer release, an
      Update button on the Settings sheet downloads the build for this
      platform from the release, verifies it against the release's
      checksum, swaps it in beside the running binary and relaunches,
      so nobody downloads and unpacks by hand again; opt-in, with the
      release page a click away as it is now. Waits on a signed build
      on Windows (Authenticode; the Mac is notarized, §440) so a swapped
      binary is not refused by the gate, and on each platform's way of
      replacing a running binary (rename and relaunch on Windows, the
      bundle swapped on the Mac, the tarball's folder on Linux); §186's
      reasons against it stand until those are in hand

#### Upstream

- [ ] The "request to hide window failed" line on every quit: Slint
      1.18.0's winit backend holds the window while it dispatches the
      close, so the check fires wrongly and the window stays
      registered; fixed in slint-ui/slint#13507 on 2026-09-19. Waits
      on the release after 1.18.0 (§99)
- [ ] Drop the rawler `[patch.crates-io]`. Waits on a rawler release
      carrying dnglab/dnglab#896 (the Deflate float writer) and #835
      (Nikon HE); the pin is the fork's float-deflate-pin, upstream
      main plus those (§524)
- [ ] rawler's DNG decoder reads the image data on a dummy read, so a
      DNG's `Stance` costs a decode; report and fix upstream, then
      drop the tag reader in `dng_mosaic` (§322)
- [ ] rawler's TIFF reader follows the next-IFD pointer with no cycle
      check when given no chain limit, so a file whose IFD0 points at
      itself grows memory until killed; its CR3 path parses the CMT
      boxes that way (§180). Report upstream with the 26-byte
      reproducer in `decode/style.rs`'s test; a visited set and a
      default chain limit is the fix
- [ ] rawler 0.8 on a CR3 with a zero-size box in `moov` loops
      until it runs out of memory, and its RW2 decoder divides by
      zero on a zero height (§160). Filed as dnglab/dnglab#848 and
      #850; the first is fixed on main by #855 (2026-10-17), in our
      pin since the same day, the second is open. The zeroed CTMD
      record of #849 is
      fixed by our #851; the NEF, RAF and RW2 panics on a zeroed tail
      are dnglab/dnglab#852, open, and the survey's guard (§324)
      catches them meanwhile
- [ ] rawler on a CR3 whose box carries a 64-bit size that wraps the
      offset past it asks for 6 GiB and aborts, on the whole file as on
      the ends (§324). #855 rejects a zero or undersized box but not a
      size that overflows the offset; report upstream as its follow-up
- [ ] ort rc.13 ignores every `ep::WebGPU` option (the key prefix is
      applied twice); greycard sets them through a config entry
      instead (§159). Fixed on ort's main in pykeio/ort@bc23566;
      drop the workaround when a release carries it
- [ ] The GPU Subject model on Windows adapters: its remaining
      Splits need nine storage buffers; an adapter allowing fewer
      falls back to the CPU. Runs on WebGPU on an M5 Mac
      (2026-10-24); check Windows (§159)
- [ ] The camera's own vignetting correction is about half of
      lensfun's on the R6 II with the RF 50mm f/1.2: measured against
      the embedded JPEG by the camera match trial
      (`docs/camera-match.md`). Decide whether the profile, a
      strength, or the maker's own table is the answer; ties to the
      next line
- [ ] The makers' embedded lens corrections, exact where a file carries
      them. Waits on rawler surfacing them (and the Panasonic lens name)
- [ ] lensfun calibrates the Sigma 50mm f/1.4 DG HSM Art with a
      distortion k1 of exactly zero (made on a Canon 6D), so it gets no
      distortion correction where Adobe's profile straightens it; measure
      it and contribute the entry to lensfun (§142)
- [ ] rawler's lens database lacks the Sigma 28mm F1.4 DG HSM Art on
      the Canon mounts and the Sony FE 24-70mm F2.8 GM II, and finds
      two definitions for one Canon ID; each decode of such a file
      warns and asks for an upstream issue. Contribute the entries
      to rawler; until then those warnings are kept out of the log

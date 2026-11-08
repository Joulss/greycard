# §604. The Develop sections told apart (2026-11-07)

Issue 80 (0.10.1): the Develop panel's sections run into one list of
controls, Look, White Balance, Camera Profile, Light, Color, Curves,
and on, and a tester finds the boundary between them by reading. The
headers are a small semibold title in capitals on the panel's own
surface, with a fold arrow, and nothing marks where a section's
controls end. Lightroom's panel sets each section off with a header
band and a border; the tester asks for some of that structure and
none of the weight.

## The decision

Two marks, both from the theme's tokens, and no change to the
controls or the spacing. The header gets a band one step up from the
panel's surface (`raised`), full width of the panel, the title and
its arrow on it as they are. A hairline in `border` runs along the
top of each section, so a folded section is a band with a line, and
the run of folded sections at the panel's end reads as a list of
headers and not as one block. Spacing stays as it is, since the
tester's one fear is losing the panel's compactness, and the band
does the telling apart on its own.

Every section comes from one component, `Section` (controls.slint),
so the change is made once and reaches the panel's sections, the
Masks tab's, and the left pane's, which share the component. The
left pane's sections take the same marks: one look for a header
everywhere. The tokens are per theme, so Charcoal, Grey, and Light
each get their own step.

## The mechanism

A `Rectangle` behind the header's row in `Section`, `Theme.raised`,
and a 1 px `Rectangle` above it in `Theme.border`; the header's own
padding unchanged. The sheets' own headers are not `Section`s and do
not change. The capture harness takes the panel before and after for
the record.

## Verified
(To be filled from the build.)

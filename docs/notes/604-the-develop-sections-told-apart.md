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

One mark, from the theme's tokens, and no change to the controls or
the spacing: a hairline in `border` along the top of each section.
Each header then sits between the line above it and the rule the
header already draws under itself, so a section reads as a block
with a lid, and the run of folded sections at the panel's end reads
as a list of headers and not as one mass. Spacing stays as it is,
since the tester's one fear is losing the panel's compactness.

We tried a band first, the header on a surface one step up from the
panel's (`raised`), and then one step down (`base`), captured on the
same frame beside the line alone. The lighter band read as a row of
buttons, cheaper than the panel around it; the darker one as a
recess that drew the eye to the headers more than to the controls.
The line alone did the telling apart and left the panel's look.

Every section comes from one component, `Section` (controls.slint),
so the change is made once and reaches the panel's sections, the
Masks tab's, and the left pane's, which share the component. The
tokens are per theme, so Charcoal, Grey, and Light each get their
own line.

## The mechanism

A 1 px `Rectangle` in `Theme.border` at the top of `Section`; the
header's own padding and rule unchanged. The sheets' own headers are
not `Section`s and do not change. The capture harness took the panel
before and after, and with each band, for the record.

## Verified
(To be filled from the build.)

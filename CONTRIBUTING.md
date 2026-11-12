# Contributing

Thanks for considering contributing! Contributions are welcomed and
appreciated. Even simple bug reports and feature requests help shape
the editor and improve it.

## Start here

Read `docs/architecture.md` first: the crates, what each may depend
on, and the develop stage by stage. The why behind every decision is
in `docs/notes.md`, one numbered section a decision. Building is in
`docs/building.md`.

## Talk before you build

A fix needs no ceremony: open the pull request. A feature or a
change of behavior starts as an issue or a discussion, so we can
shape how it fits before the work is done, and no two people build
the same thing. The roadmap (`docs/roadmap.md`) says what is planned
and what each item waits on.

## The rules of the code

- `cargo test`, `cargo clippy --all-targets`, and `cargo fmt`, all
  clean. The GPU tests also run on a software Vulkan driver such as
  Mesa's lavapipe, which finds what two vendors hide.
- Every pipeline op has a CPU reference with a test. A GPU
  implementation is checked against it, never the other way.
- `greycard-core` never depends on a UI toolkit, a display rendering
  (the curve or look that turns its linear output into a picture), or
  the edit schema. Decoding is rawler's: a camera fix is written
  against upstream and sent there; our fork pin only carries it until
  a release does.
- Nothing binary in the repo: no raws or slices of them, no JPEGs,
  no models. Fixtures are scrubbed heads: a raw's first kilobytes, cut
  at a stated byte count, with the preview and the serial numbers
  zeroed. A sample for a new body goes to raw.pixls.us, with the link
  in the issue.
- The UI's wording is uneven and we want it consistent, so a rename
  or new text starts as an issue rather than a pull request. Help
  tidying it is welcome. Log lines are free.
- American spelling ("grey" excepted), and the Oxford comma.

## A pull request

One commit carries the code and its notes section. The subject is
short and imperative; the body says why.

The note goes in as `docs/notes/<slug>.md`, with no number in its
name or its title. The changelog lines for a tester go in the pull
request's description. Leave `docs/notes.md`, `docs/changelog.md`,
and `docs/roadmap.md` alone: every open pull request would edit the
same lines of them, and two would pick the same next number. At the
merge I number the note, add its index line, place the changelog
lines under the version, and take the roadmap line off.

## License and origin

Everything is GPL-3.0-or-later, so RawTherapee and darktable
algorithms can be ported. Ported code says in its file header where it
came from and who wrote it. Your contribution comes in under the same
license, with no agreement to sign. Sign each commit off
(`git commit -s`), which adds a `Signed-off-by` line: the
[Developer Certificate of Origin](https://developercertificate.org),
your word that the code is yours to give.

Write with any tool you like. You answer for every line, and the
three checks above and the reference tests decide.

## Reporting

A bug: the version, the log, and what you did. A camera: the sample's
link. A security problem: email <jess@jessolmstead.com> rather than
opening an issue.

## Conduct

Be civil and specific: criticize the picture or the code, never the
person. Testers' photographs and names stay theirs; nothing from an
issue is reposted elsewhere. Harassment, slurs, and personal attacks
are out, in issues, discussions, and commits alike. The maintainer
decides, from a deleted comment to a block. Reports go to the address
above.

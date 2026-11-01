# §471. The rest of the windowless modules

*2026-10-31*

§467 left six modules in the binary, each tied to the window by a
little. Each got its one real change and moved into the library.

**The changes.** Delete took the system trash as an argument from
its three callers, so the test-only stand-in lives at the window's
edge and the library cannot reach the trash on its own. The GPU
marker's tests make their own scratch. Zoom took cull's tile
arithmetic, and cull reads it back by re-export. Collections' store
lost the window's two models and the sheet's question, which are a
window-side `Held` now, so a store reload no longer carries the
models across by hand; Forget missing reads the version's sidecar
itself, the same answer the export queue gave. Import's four
helpers of the panel's sync and startup are a library module,
`preset_fit`, re-exported at their old paths. Cull's rejects half
is a library module, `rejects`, and archive followed it. Every test
name as before, 1653 in all, 167 in the library now.

**The hazard, in a second form.** §467 named the stand-in that
stops applying. The other form: a `cfg(test)` item the binary's
tests name is invisible when the binary's tests build the library,
so the build breaks, or a seam goes unused. Three were made
unconditional (collections' `Added` and `DIR`, archive's `no_beat`);
the tamper seam that only the library's own test sets stays
`cfg(test)`, one shape. Camera match and the measurement runners
take the worker's frame develop and wait on the engine's part 2.

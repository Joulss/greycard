# §564. Lenses sharing an id, told apart

*2026-11-04*

A tester's Rebel T8i frame on a Sigma 17-70mm said "not recorded"
for its lens while Affinity named it (discussion 59). The file does
name it: Canon lens id 131, and the EXIF LensSpecification 17 to
70 mm. Canon gives third-party lenses one id, and rawler's table has
nine lenses on 131. The resolver narrows by the focal length the
frame was shot at, 17 mm, which leaves three, and gives up with
"Found multiple (3) lens definitions". It never read the
LensSpecification, which is the lens's own range and settles it.

The fix is in rawler (dnglab/dnglab#929, on the fork's pinned
branch until a release carries it): the CR3 and CR2 decoders hand the
specification to the resolver, and among several candidates the
entry whose focal range it is wins. With no match, or more than one,
the resolver does as before, so nothing that resolved yesterday
resolves differently. The tester's frame reads
"Sigma 17-70mm f/2.8-4.5 DC Macro".

The same tester's Adobe profile for that lens parses whole; what
they lacked was the Lens picker that lists the profile folder's
LCPs, §516, which 0.9.1 had not got. Nothing to fix there.

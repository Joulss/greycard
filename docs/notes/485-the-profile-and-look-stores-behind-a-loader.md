# §485. The profile and look stores behind a loader

*2026-10-31*

The last of §463's split, the seventh item of the 2026-10-29 layering
review. greycard-edit read the camera profile and look directories
itself: `settings()` and `look_under()` loaded files, and the develop
key hashed them. Now the edit holds data and conversion only.

**The loader.** A `Loader` trait in greycard-edit, `profile(name)`
and `look(name, strength, curve)`, with a `NoStores` that answers
nothing for tests that develop without files. `Edit::settings`,
`Camera::profile`, and `LookLut::look_under` take `&dyn Loader`;
render's export takes one after the edit. The listing, the paths, the
caches, and the forgetting moved to greycard-edit-io as `profiles`
and `looks`, with `Stores { profiles, looks }` implementing the trait
and `Stores::user()` resolving the user's two directories, two paths
and no read, under half a microsecond. The caches are the statics
they were, keyed and kept the same; a warm settings and look resolve
costs what it did.

**The key went too.** The develop key hashed files from the same two
folders, so it could not stay behind: greycard-edit keeps the hash in
`Edit::develop_key_with(&[key::Read], lenses)`, edit-io reads the
files and hands them in, and the keys are byte for byte what they
were, pinned against the old crate on a profile, a look, a lens
database, and a missing file. The preset store and the sidecar's own
reads stay, outside this item.

**The engine's sites** build `Stores::user()` where they call
`settings()`, one line each, so the develops' move into walks (§478's
last step) rebases over this without a fight. Handing one loader down
from the window through the worker follows once that move lands.

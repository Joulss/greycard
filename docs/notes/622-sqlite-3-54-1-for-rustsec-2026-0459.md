# §622. SQLite 3.54.1, for RUSTSEC-2026-0459 (2026-11-09)

**CI's audit stopped on the library's SQLite.** rusqlite 0.37 bundles
SQLite 3.53, and RUSTSEC-2026-0459 (CVE-2026-119375) is against every
SQLite before 3.54.0: a database opened beside a `-wal` file whose
page size differs from the database's can return uninitialized
page-cache memory to an ordinary query. Local, and the files must be
opened; our index is written by us under the user's own cache
directory, so the exposure is a crafted pair of files put there by
another process. Low for us, but the audit gate is a gate.

**rusqlite 0.41.1.** libsqlite3-sys 0.39.1 bundles SQLite 3.54.1. The
one change in the way: `Connection::busy_handler` now takes `&mut
self`, so `open_with_busy` binds its connection mutably and
`Library::set_busy_handler` takes `&mut self`; its two callers in the
index's lanes already own their `Library`. The fuzz lock moves with
the workspace's, as every bump must, or CI's fuzz job fails on `--locked`.

Tests: the workspace's, unchanged; `cargo audit` clean.

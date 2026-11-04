# §558. A rename into place waits out a hold

*2026-11-05*

On Windows a rename over a file another process has open fails: a
sync client or a scanner reading the old sidecar refuses the save for
the few milliseconds it holds it, and a merge made again over a file
something still reads may refuse `promote` (§541, §543).

**One rename.** `greycard_core::fs::rename_into_place` renames, and
on Windows tries again on `ERROR_ACCESS_DENIED` (5),
`ERROR_SHARING_VIOLATION` (32), and `ERROR_LOCK_VIOLATION` (33) only:
ten tries, the waits growing by 40 ms, 1.8 s in all. Any other error,
or the last refusal, is returned as before. It logs once at the end
when it waited. Elsewhere a rename over an open file succeeds, so it
is a plain rename. The sidecar's commit (so every synced save and the
rejects queue), the sidecar's move between its two places, and
`promote` call it. A save on the window's thread can now wait up to
1.8 s on a hold where it failed at once.

**The tests' rename** (`testing::rename_retry`) runs the same loop on
any error for 20 s: a test's own pool can hold a folder longer than
another process's moment. One loop, two schedules.

The retry is tested with a stand-in rename (held twice then let go,
held every time); the Windows path itself is untried until a Windows
run meets a hold.

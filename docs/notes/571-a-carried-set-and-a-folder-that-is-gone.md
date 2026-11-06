# §571. A carried set and a folder that is gone

*2026-11-05*

The carrier writes a set beside its frames, in every folder a ref's
hint names, and tries a write that failed again every minute (§244).
A frame moved on since leaves its hint pointing at a folder that is
not there, and the log showed one set asked of a rejects folder that
no longer exists, every 60 seconds, for the whole session.

A folder that is not there is not a share that did not answer: it is
a stale hint, and the set's next revision names where the frame went.
The write now counts that folder as done and says "not tried again";
a share that fails otherwise is retried as before.

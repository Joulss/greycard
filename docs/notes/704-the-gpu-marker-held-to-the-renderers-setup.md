# §704. The GPU marker held to the renderer's setup (2026-11-16)

§395's marker missed a second Windows machine (Intel UHD, legacy driver
line 31.0.101.2115, two monitors): it died at every start on 0.11.0 and the
next start was not sent to DX12. `WGPU_BACKEND=dx12` by hand worked.

**The order of a start.**

On the window's thread, in `panel::startup` (the same in v0.11.0 and now):

1. `gpu_marker::start`: the lock taken, the marker written (`setup: default`).
2. Slint's select (`BackendSelector::select`).
3. In 0.11, `gpu_marker::selected`: the marker taken off (a clean start) and
   the lock let go.
4. `App::new`, the wiring, then `app.run()`: the event loop.
5. winit's `resumed`: `window_fit::Handler` fits the window and logs
   `window: placed at …`; Slint then creates the window, and with it the
   real wgpu instance, the surface, the adapter, and the device.
6. The first frame Slint draws: once a surface texture is acquired, its
   rendering setup, where `gpu: …` is logged.

The dead starts' last line is step 5's and the `gpu:` line never came, so
the driver died between 5 and 6, after step 3 had taken the marker off. That
is why the cache held the lock file and no marker: the gap the header named,
a driver that dies only as the real device is made. Not before the marker was
written (`window: placed` comes long after it), and no other path removes it.

**The fix.**

The marker and its lock are held from before the select to the end of the
renderer's setup: after the `gpu:` line, the viewport's pipelines, and the
device handed to the worker, whatever the graphics API. There it is taken
off on the default backends, or on DX12 made the pin. The select's hook now
leaves the marker on. The restored frame's open note (`open_note`) was
written right after the select, which would have counted one death twice;
it is now written at the same point, before any frame opens.

**The pin.**

A pin is `died: default` and `drew: dx12`, written only once DX12 has set a
renderer up on the machine: proof DX12 draws there. A DX12 start from a pin
writes `setup: dx12` beside `drew: dx12`, and a death or a panic before its
first frame leaves that, which the next start reads as the pin kept, with a
warning, not as DX12 dead. Without the `drew:` line, a machine whose default
backends kill the process was unpinned by any death on DX12 before its first
frame (a kill, a logoff, a shutdown, out of memory, a panic of ours, on a
window started minimized or not), and every start after died on the default.

Two things undo a pin: deleting the file, which the log names at every start
that reads it, and DX12 failing with evidence (its select an error, or its event loop
ending in an error before the setup). On DX12's first start, before it has
drawn, a death or a panic still counts against it, and a death on the default
after that is the end of trying: the start goes on with the default, with a
warning, rather than turn between the two.

A file without the `drew:` line reads as before: 0.11's pin (`died: default`)
sends starts to DX12, and its first DX12 start that sets up adds the line.
Markers from this branch's earlier commits have the same form (none are in the
wild).

What a start leaves, by how it ends (`found` is the marker as read):

| ends                                  | on the default    | on DX12 we set     |
|---------------------------------------|-------------------|--------------------|
| renderer set up, then anything        | nothing           | the pin            |
| dies before it                        | `setup: default`  | `setup: dx12`      |
| panics before it                      | `setup: default`  | `setup: dx12`      |
| returns early, an error of its own    | `found`           | `found`            |
| event loop ends cleanly, no frame     | `found`           | `found`            |
| event loop ends in an error, no frame | `found`           | DX12 failed        |
| select on DX12 an error               | n/a               | DX12 failed, the default selected in-process |

`setup: dx12` keeps a pin's `drew: dx12`, and is then no death. An event loop
that ends without a frame drawn is no evidence: a window started minimized or
under a locked session is never drawn. A lost device in the setup that
`gpu_lost::survive` catches is a clean end too.

A panic before the setup counts as a death: wgpu's errors on the window's
thread panic (`gpu_errors`), so a shader the driver will not compile would
otherwise recur on every start without DX12 tried. On Windows winit catches a
panic in its callbacks and resumes it out of `run` (`runner.catch_unwind`,
`take_panic_error`), `survive` re-raises it, and it unwinds through the
marker's guard in `startup::main` (the profile unwinds). A panic of ours that
is no GPU's counts too: on the default on Windows a pin to DX12, elsewhere a
warning.

A start with `WGPU_BACKEND` set by hand writes nothing to the marker. A
`drew:` line names only DX12; anything else in it is ignored.

A test enumerates every sequence of four endings from every marker state (and
odd ones: no file, empty, garbage, the header alone, lines not ours, a pin
with Windows line ends, and 0.11's forms), in memory, on a machine whose
default backends kill and a healthy one, on Windows and off:
the killer machine is on DX12 at the fifth start unless DX12 failed with
evidence or died before it ever drew there; the healthy one leaves the default
only after a death or a panic; off Windows nothing is set. It fails on both
earlier versions of this change, and takes about 2 s with no file I/O. The
guard's two branches and the loop end's two are driven through a real unwind
against a folder, and the lifecycle tests cover the file's side.

**The cost.**

The window is not short. It runs through `App::new`, the wiring, and the first
frame Slint draws, which comes only once a surface texture is acquired (1.3 s
on a healthy machine in a tester's log), and Slint's femtovg renderer skips
frames on a zero-size or occluded surface, so a start minimized or under a
locked session holds the marker until it is first drawn. A start killed,
logged off, shut down, or out of memory in it reads as a death. winit 0.30
hears no `WM_ENDSESSION`, so the process ends without our drop; catching it
would take a window-message hook of our own, not built.

On the default backends such a false death pins that Windows machine to DX12
until the file is deleted: nothing else makes greycard try Vulkan again. DX12
draws, so the cost is the backend, not the editor. On a pinned machine a false
death costs nothing. No user doc mentions DX12 or the marker.

**On the machine.**

The next start finds no marker (0.11.0 left none), so it dies once more on
Vulkan with the new build, leaving `setup: default`; the start after goes to
DX12, and once it draws the pin holds. A `WGPU_BACKEND` the tester set by hand
stays theirs.

**Not done.**

DX12 first for Intel's legacy Vulkan line was weighed: telling the line apart
before any Vulkan driver loads means reading the display adapter's driver
version from the registry and guessing a version boundary we cannot test
here, and it would move working machines off Vulkan.

**What it still cannot catch.**

A driver that dies after the setup: drawing a frame (Slint's own pipelines
are built on its first), or setting up again after a lost device. A driver
that loses the device in the setup on every start, rather than killing the
process: the event loop survives the loss as a clean end, so the machine
never reaches DX12 unless the loss surfaces as a panic before
`gpu_lost::install`.

Nor does it give up on a backend that keeps dying. A pinned machine whose
DX12 later dies or panics before the setup on every start stays on DX12
until the file is deleted; the likeliest route is a healthy machine pinned by
a false death, then a failure of DX12's alone. And a machine with both
backends counted dead runs the default for good, so one where only the
default kills is stranded there by a single failed event loop or select on
DX12, a death on DX12's first start, or a downgrade to 0.11 and back (0.11
ignores `drew:`). Every warning that reports a death or a pin kept names the
file and says deleting it starts over, so the way back is in the log where it
matters.

**Next.**

- A bound on a backend that keeps dying: a `kept: N` count beside a pinned
  backend's setup line (and for the both-dead state), so after a few deaths in
  a row on a pin the start tries the other backends again instead of staying
  for good. Not in this change: it is a new rule, and the both-dead state's
  answer (which backend to try, how often) wants its own decision.
- Write the marker by write-then-rename. Today `apply` writes in place, and a
  kill inside that write can leave a torn prefix; of a pin's rewrite, 25
  prefixes strand a machine (read as a death on the wrong backend or no pin).
  It needs a kill inside a write of a few dozen bytes, so it is rare, but the
  fix is small.

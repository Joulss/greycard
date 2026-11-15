# §684. A setup that starts no shell (2026-11-15)

Testers of the zip said the install's command window "feels like
being hacked", and one antivirus blocked it. The zip's `install.cmd`
shows a console and runs PowerShell with the policy bypassed; the
setup (§619) did the same out of sight, `{cmd} /c register.cmd`, which
ran `reg add` a dozen times and then PowerShell compiling a `shell32`
call. An unsigned installer starting a hidden shell that starts
PowerShell with the policy bypassed is the pattern behavior heuristics
are written for. §619 is out of date where it says the setup runs and
installs the two scripts.

## The setup

- The registration is `[Registry]` entries, key for key what
  `register.cmd` writes under `HKCU\Software\Classes`. `[Run]` keeps
  only the launch at the end; `[UninstallRun]` is gone.
  `ChangesAssociations=yes` has Inno tell Explorer itself. The setup
  starts no cmd, reg, or PowerShell, and the check on CI reads both
  Inno logs to say so.
- Uninstall takes our own keys whole (`greycard.gcd`,
  `greycard.photo`, `Applications\greycard-ui.exe`) and, in shared
  keys, our value alone: an extension's `OpenWithProgids` goes only if
  that leaves it empty, and the extension's key is never flagged. The
  check plants another program's offer on `.cr3` and expects it after
  install and after uninstall.
- `.gcd`'s default value is removed in `[Code]` at `usUninstall`, only
  while it still reads `greycard.gcd`. That is gentler than
  `unregister.cmd`, which deletes the whole key.
- The offers are empty `REG_SZ` values where `register.cmd` writes
  `REG_NONE`: Inno's `ValueType: none` creates the key and no value.
  Windows reads only the names.
- The two scripts are not installed, are deleted if found
  (`[InstallDelete]`), and are deleted at uninstall
  (`[UninstallDelete]`) in case a zip install put them back.

## The zip

It keeps the scripts. Their PowerShell line is gone: `install.ps1`,
already PowerShell, sends the association change itself. Its `-Remove`
falls back to the `unregister.cmd` beside it when the installed one is
missing, so the zip's uninstall after a setup install still
unregisters. Run by hand, `register.cmd` registers at once and
Explorer's icons catch up at sign-out; cmd has no plain way to send
the notice.

## The editor

`greycard-ui.exe` is a windowed program and starts no console program
on Windows: Explorer, the browser, and itself after a lost device have
no console, and Edit in runs what the user chose, console and all. The
one console child is `nvidia-smi`, built only with the `cuda` feature
no package turns on; it is windowless now (`windowless` in
`greycard-ai`'s runtime). Nothing of ours or a dependency's allocates
a console.

## The properties

Both executables carry CompanyName `Jess Olmstead` and LegalCopyright
`Copyright © Jess Olmstead. GPL-3.0.`, and the setup the same: a
package links Slint under GPL-3.0-only, so not "or later". Inno Setup,
the Pascal Script it carries, and its LZMA decoder are in the bundled
notices, at the pinned version's texts.

## Known edges

- A setup built from §619 as it first stood left `unregister.cmd` in
  its uninstall log, which no later setup can remove: that uninstall
  starts one hidden cmd, which fails on the deleted script and changes
  nothing. No release carried such a setup.
- A zip install over a setup install leaves the setup's entry in
  Installed apps; the zip's uninstall removes the folder, not the
  entry.
- `greycard.exe`, the command line, sits beside the editor and on the
  Path: double-clicked, or run from Win+R, it is a console that opens
  and closes.

## Unverified here

Nothing was compiled for Windows: the `.iss`, the © through
winresource and Inno, the PowerShell, and the order Inno uninstalls
in all wait on a run of `windows-package.yml`. Whether an antivirus
still flags the unsigned setup on reputation alone waits on a scan;
signing is the real answer. What else such an engine may weigh: the
ONNX Runtime loaded at a model's first use, and where the shipped one
cannot run, downloaded on request and loaded; the update check at
launch; the Path edit.

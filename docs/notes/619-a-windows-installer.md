# §619. A Windows installer (2026-11-09)

The Windows release gains `greycard-<version>-<arch>-windows-setup.exe`
beside the zip, on both Windows legs. The zip stays: two testers would
rather unzip and run in place, and `install.cmd` stays in it.

**Inno Setup, not WiX.** What we install is one user's: no elevation,
one directory, a Start menu entry, a Path entry, and associations
under `HKCU`. That is Inno Setup's ordinary case
(`PrivilegesRequired=lowest`) and one script. An MSI built by WiX can
be per user, but the MSI rules on components, key paths, and per-user
versus per-machine folders are written for the other case, and a
per-user MSI that later grows an all-users mode is a second package.
Inno's `{autopf}`, `{autoprograms}`, and
`PrivilegesRequiredOverridesAllowed` cover the files and the Start
menu for the all-users install the roadmap holds. The rest needs work
of its own: the machine's Path under `HKLM`, and the associations,
since `register.cmd` and `unregister.cmd` write `HKCU\Software\Classes`
by name.

**The same install, the same way.** `packaging/windows/greycard.iss`
installs what `install.ps1` installs, where it installs it: the
staging directory's `bin\` as `%LOCALAPPDATA%\Programs\greycard` with
`LICENSE`, `README.md`, `register.cmd`, and `unregister.cmd` beside
it, `greycard.lnk` in the Start menu's Programs folder with the same
target, working directory, and comment, and the directory on the
user's Path. The associations are `register.cmd`'s, run by the
installer as `install.ps1` runs it, and `unregister.cmd` runs first
on uninstall, so the list of raw extensions stays where it is rather
than gaining a third copy in `[Registry]` lines. The Path is read
raw, as `install.ps1` reads it, since Inno's `RegQueryStringValue`
does not expand, so a `%USERPROFILE%` entry survives; it is compared
without case or a trailing backslash, and taken back out on
uninstall. A Path that is not a string is left alone rather than
overwritten. Inno writes the uninstall entry, so greycard is in
Settings > Apps > Installed apps.

The differences, all small. The Path is written back as
`REG_EXPAND_SZ` whatever kind it was, because Pascal Script cannot
read a value's kind; Windows makes a user Path that kind anyway. A
running greycard is offered to be closed through the Restart Manager
rather than refused. The setup asks nothing; `/DIR=` on its command
line does what `GREYCARD_PREFIX` does for the script. Uninstalling
removes the files the installer put there and the directory if it is
then empty, where `uninstall.cmd` removes the directory whole. And
the closing words about settings left in place are the script's own;
the uninstaller says only Inno's standard ones.

**Built from the zip's staging directory.** `scripts/windows-setup.ps1`
compiles the script over the directory `package.sh` staged for the
zip, so the two carry the same bytes. The version is defined on the
compiler's command line from Cargo.toml, which the tag already has to
match; the file version resource takes its first three numbers, since
it holds digits alone. Inno Setup 7.1.1 is fetched from its GitHub
release and checked against the SHA-256 listed there
(`scripts/inno-setup.ps1`). The runner images carry an Inno Setup 6
at whatever version each was built with; a pinned compiler builds the
same setup from the same tree.

Each leg then installs the setup on its runner, checks every file,
`greycard --version` from the installed copy, the shortcut's target,
the Path (with a `%USERPROFILE%` entry planted first), every
association, and the Installed apps entry, uninstalls it, and checks
each is gone (`scripts/windows-setup-check.ps1`). Any failure prints
the setup's and the uninstaller's logs, and fails the run rather than
a tester.

The ARM leg builds one too. The script is the same with `arm64` for
`x64compatible`, the round trip runs natively there, and the leg is
already allowed to fail. The x64 setup is allowed on Windows on ARM
on purpose (`x64compatible`, and the same AppId, so either replaces
the other): when the aarch64 leg fails a release, the x64 setup is
what an ARM user has. Setup itself stays Inno's default 32-bit x86
program rather than `SetupArchitecture=x64`, since that is the one
build that runs everywhere both packages install.
`windows-package.yml` builds the x86-64 zip and setup by hand from
any branch, as `arm-package.yml` does for ARM, which now builds its
setup too.

**Signing waits on a service.** One step in `release.yml`, marked, is
where a signing service goes. It sets `WINDOWS_SIGN_SCRIPT`, the
Windows path of a PowerShell script that signs the files it is given,
as the Mac's signing step sets `MACOS_SIGN_IDENTITY` for
`package.sh`. `package.sh` signs both executables with it before the
zip, and the installer script hands it to Inno as its sign tool,
which signs the setup, the uninstaller, and the setup's temporary
copies of itself; a setup signed after the compile would leave the
uninstaller unsigned. Without the secret the step does nothing and
both go out unsigned; with the secret and no service wired in, it
fails rather than ship unsigned quietly. A service that signs per
file through `signtool` (Azure Artifact Signing, or a CA's
certificate in a cloud key store) fits this as it is. One that takes
an artifact and returns it signed, as SignPath does, needs the
uninstaller signed in a first pass (`SignedUninstallerDir`) and the
setup in a second.

**Before a tag.** We wrote this with no Windows at hand, so the first
compile is a run of `windows-package.yml`. It must be green before a
release tag: a setup failure on the x86_64 leg fails its build job,
and so the release.

# Fetch and install the Inno Setup compiler windows-setup.ps1 uses,
# pinned and checked, for a runner.
#
#     powershell -File scripts/inno-setup.ps1 -Destination <dir>
#
# Inno Setup 7.1.1, the release jrsoftware recommends, from its GitHub
# release, checked against the SHA-256 that release lists. GitHub's
# Windows images carry an Inno Setup 6 (6.7.1 on windows-2025, 6.6.1
# on windows-11-arm, by their readmes in November 2026) at whatever
# version each image was built with; this goes beside it, for the
# current user, into -Destination. The x64 build runs on Windows on
# ARM too, under emulation. On a runner the compiler's path goes into
# $GITHUB_ENV as ISCC, where windows-setup.ps1 looks first.

[CmdletBinding()]
param([Parameter(Mandatory)][string]$Destination)

$ErrorActionPreference = 'Stop'

# Windows PowerShell's progress bar slows Invoke-WebRequest to a crawl.
$ProgressPreference = 'SilentlyContinue'
$url = 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x64.exe'
$sha256 = '8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0'

$exe = Join-Path ([System.IO.Path]::GetTempPath()) 'innosetup-7.1.1-x64.exe'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri $url -OutFile $exe -UseBasicParsing
$got = (Get-FileHash $exe -Algorithm SHA256).Hash
if ($got -ne $sha256) { throw "inno-setup.ps1: $url has SHA-256 $got, not $sha256" }

$p = Start-Process $exe -Wait -PassThru -ArgumentList @(
    '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/CURRENTUSER',
    "/DIR=`"$Destination`"")
if ($p.ExitCode -ne 0) { throw "inno-setup.ps1: the Inno Setup install exited with $($p.ExitCode)" }
Remove-Item $exe

$iscc = Join-Path $Destination 'ISCC.exe'
if (-not (Test-Path $iscc)) { throw "inno-setup.ps1: no $iscc after the install" }
& $iscc --version
if ($env:GITHUB_ENV) {
    "ISCC=$iscc" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding ascii
}

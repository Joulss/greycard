# Compile the Windows installer from what scripts/package.sh staged.
#
#     powershell -File scripts/windows-setup.ps1 [-Version <version>] [-Iscc <path>]
#
# Run after package.sh on Windows. It compiles
# packaging/windows/greycard.iss over the staging directory
# package.sh left beside the zip, target/dist/greycard-<version>-<arch>-windows,
# so the setup and the zip carry the same files, and writes
# target/dist/greycard-<version>-<arch>-windows-setup.exe.
#
# The version is the workspace's, as package.sh reads it; an argument
# has to agree with it, for the reason package.sh gives. The compiler
# is -Iscc, else $env:ISCC, else ISCC.exe on the Path, else Inno Setup
# 7's default install directories.
#
# When $env:WINDOWS_SIGN_SCRIPT holds the Windows path (not a Git Bash
# /d/a/... one) of a PowerShell script that signs the files
# it is given, the compiler gets it as the sign tool named greycard and
# signs the setup and its uninstaller with it. The release workflow's
# signing step is what sets it; package.sh signs the binaries with the
# same script before it zips them.

[CmdletBinding()]
param(
    [string]$Version,
    [string]$Iscc
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$crate = ((cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).packages |
    Where-Object { $_.name -eq 'greycard-ui' }).version
if (-not $crate) { throw 'windows-setup.ps1: cargo metadata has no version for greycard-ui' }
if (-not $Version) { $Version = $crate }
if ($Version -ne $crate) {
    throw "windows-setup.ps1: asked for $Version, but the workspace is at $crate."
}

# The host's triple names the package, as in package.sh, and says
# which Windows the setup may install on.
$hostTriple = (rustc -vV | Select-String '^host: (.*)$').Matches[0].Groups[1].Value
$arch = $hostTriple.Split('-')[0]
$allowed = switch ($arch) {
    'x86_64' { 'x64compatible' }
    'aarch64' { 'arm64' }
    default { throw "windows-setup.ps1: no installer for host $hostTriple" }
}

$name = "greycard-$Version-$arch-windows"
$stage = Join-Path (Get-Location).Path "target\dist\$name"
if (-not (Test-Path (Join-Path $stage 'bin\greycard-ui.exe'))) {
    throw "windows-setup.ps1: $stage has no bin\greycard-ui.exe; run scripts/package.sh first."
}

if (-not $Iscc) { $Iscc = $env:ISCC }
if (-not $Iscc) {
    $found = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($found) { $Iscc = $found.Source }
}
if (-not $Iscc) {
    $Iscc = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 7\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $Iscc) { throw 'windows-setup.ps1: no ISCC.exe; install Inno Setup 7 or pass -Iscc.' }

# The file version resource takes digits alone: 0.12.0-rc.1 is 0.12.0.
$numeric = ($Version -split '[-+]')[0]

$arguments = @(
    "--define=Version=$Version",
    "--define=NumericVersion=$numeric",
    "--define=Stage=$stage",
    "--define=Arch=$allowed",
    "--output-dir=$((Resolve-Path 'target/dist').Path)",
    "--output-filename=$name-setup"
)
if ($env:WINDOWS_SIGN_SCRIPT) {
    $sign = (Resolve-Path $env:WINDOWS_SIGN_SCRIPT).Path
    # $q and $f are the compiler's: a quote, and the quoted file to sign.
    $arguments += '--define=Sign'
    $arguments += '--signtool=greycard=powershell.exe -NoProfile -ExecutionPolicy Bypass -File $q' + $sign + '$q $f'
}
$arguments += 'packaging/windows/greycard.iss'

& $Iscc --version
& $Iscc @arguments
if ($LASTEXITCODE -ne 0) { throw "windows-setup.ps1: ISCC failed with exit code $LASTEXITCODE" }
Write-Host "target/dist/$name-setup.exe"

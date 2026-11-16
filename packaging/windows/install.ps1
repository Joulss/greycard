# Install greycard for one user. No elevation, nothing outside the
# user's own profile. install.cmd and uninstall.cmd are the two ways in;
# this is the work they both do.
#
# Everything lands in %LOCALAPPDATA%\Programs\greycard, which is where
# a per-user application goes on Windows and is already excluded from
# the places an administrator locks down. The binaries look for
# onnxruntime.dll beside themselves and nowhere else, and it finds
# webgpu_dawn.dll in the executables' directory, so the whole of
# bin\ moves as one directory rather than the executables going to one
# place and the libraries to another.
#
# Set GREYCARD_PREFIX to install somewhere else.

[CmdletBinding()]
param([switch]$Remove)

$ErrorActionPreference = 'Stop'

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$target = if ($env:GREYCARD_PREFIX) {
    $env:GREYCARD_PREFIX
} else {
    Join-Path $env:LOCALAPPDATA 'Programs\greycard'
}
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$shortcut = Join-Path $startMenu 'greycard.lnk'

# The user's Path lives in the registry as a REG_EXPAND_SZ holding
# things like %USERPROFILE%\bin. Reading it back through
# [Environment]::GetEnvironmentVariable expands those before handing
# them over, and writing the expanded string back is how a Path loses
# its variables for good. So the value is read and written through the
# registry with expansion off, and its own kind is kept.
function Get-UserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    if (-not $key) { return @{ Key = $null; Value = ''; Kind = 'ExpandString' } }
    $value = $key.GetValue(
        'Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    $kind = try { $key.GetValueKind('Path') } catch { 'ExpandString' }
    @{ Key = $key; Value = [string]$value; Kind = $kind }
}

# A new shell reads the Path at startup; the ones already open are told
# by this broadcast, which is what the System control panel sends when
# someone edits it there. It is a courtesy, so it is allowed to fail.
function Send-SettingChange {
    try {
        if (-not ('G.Env' -as [type])) {
            Add-Type -Namespace G -Name Env -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, IntPtr wParam,
    string lParam, uint fuFlags, uint uTimeout, out IntPtr lpdwResult);
'@
        }
        $out = [IntPtr]::Zero
        # HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG, 3 seconds.
        [G.Env]::SendMessageTimeout(
            [IntPtr]0xffff, 0x1a, [IntPtr]::Zero, 'Environment', 2, 3000, [ref]$out) | Out-Null
    } catch { }
}

# Explorer caches associations; after register.cmd or unregister.cmd
# this tells it to look again (SHCNE_ASSOCCHANGED), from this process,
# since a batch file cannot make the call. A courtesy too: the entries
# are on disk either way and a sign-out picks them up.
function Send-AssocChanged {
    try {
        if (-not ('G.Shell' -as [type])) {
            Add-Type -Namespace G -Name Shell -MemberDefinition @'
[DllImport("shell32.dll")]
public static extern void SHChangeNotify(int wEventId, uint uFlags, IntPtr dwItem1, IntPtr dwItem2);
'@
        }
        [G.Shell]::SHChangeNotify(0x8000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
    } catch { }
}

if ($Remove) {
    # A GREYCARD_PREFIX set wrong must not take another folder with it:
    # only one holding a binary the install puts there (either, since a
    # removal stopped by the other running may have taken one), and
    # never a drive's root or the user's profile.
    $full = [IO.Path]::GetFullPath($target).TrimEnd('\')
    $root = [IO.Path]::GetPathRoot($full).TrimEnd('\')
    $ours = (Test-Path (Join-Path $target 'greycard-ui.exe')) -or
        (Test-Path (Join-Path $target 'greycard.exe'))
    $notOurs = ($full -eq $root) -or ($full -eq "$env:USERPROFILE".TrimEnd('\')) -or
        ((Test-Path $target) -and -not $ours)
    if ($notOurs) {
        Write-Host "  $target is not a greycard install; nothing removed" -ForegroundColor Yellow
        exit 1
    }

    Write-Host "removing greycard from $target"

    # The installed copy first; the setup installs none, so over a
    # setup's install the one beside this script does the same work
    # (it names no path of its own).
    $unregister = @((Join-Path $target 'unregister.cmd'), (Join-Path $here 'unregister.cmd')) |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($unregister) {
        & cmd.exe /c "`"$unregister`""
        Send-AssocChanged
    }

    if (Test-Path $shortcut) {
        Remove-Item $shortcut -Force
        Write-Host "  $shortcut"
    }

    $path = Get-UserPath
    if ($path.Key) {
        $kept = @($path.Value -split ';' | Where-Object { $_ -and $_.TrimEnd('\') -ne $target.TrimEnd('\') })
        if ($kept.Count -ne @($path.Value -split ';' | Where-Object { $_ }).Count) {
            $path.Key.SetValue('Path', ($kept -join ';'), $path.Kind)
            Write-Host "  taken off your Path"
            Send-SettingChange
        }
        $path.Key.Close()
    }

    if (Test-Path $target) {
        try {
            Remove-Item $target -Recurse -Force
            Write-Host "  $target"
        } catch {
            Write-Host "  $target is in use; close greycard and run this again" -ForegroundColor Yellow
            exit 1
        }
    }

    Write-Host "done. Your settings, presets and cached models are untouched;"
    Write-Host "they are under %APPDATA%\greycard and %LOCALAPPDATA%\greycard."
    exit 0
}

$source = Join-Path $here 'bin'
if (-not (Test-Path (Join-Path $source 'greycard-ui.exe'))) {
    throw "install.ps1: run this from the unpacked greycard archive; there is no bin\greycard-ui.exe beside it."
}

Write-Host "installing into $target"

# Copying over a binary that is running fails partway and leaves the
# directory half old and half new, so the running one is found first
# and said so plainly.
foreach ($name in 'greycard-ui', 'greycard') {
    if (Get-Process -Name $name -ErrorAction SilentlyContinue) {
        throw "install.ps1: $name is running. Close it and run this again."
    }
}

New-Item -ItemType Directory -Force -Path $target | Out-Null
Copy-Item (Join-Path $source '*') $target -Recurse -Force
foreach ($name in 'LICENSE', 'THIRD-PARTY-NOTICES', 'README.md', 'register.cmd', 'unregister.cmd') {
    $file = Join-Path $here $name
    if (Test-Path $file) { Copy-Item $file $target -Force }
}
Get-ChildItem $target -File | ForEach-Object { Write-Host "  $($_.Name)" }

# The Start menu entry. greycard-ui carries its own icon as a resource,
# so the shortcut needs no .ico pointed at it.
New-Item -ItemType Directory -Force -Path $startMenu | Out-Null
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
$link.TargetPath = Join-Path $target 'greycard-ui.exe'
$link.WorkingDirectory = $target
$link.Description = 'A raw editor that gets the light right'
$link.Save()
Write-Host "  $shortcut"

# The command line, by name, in a shell opened after this.
$path = Get-UserPath
if ($path.Key) {
    $entries = @($path.Value -split ';' | Where-Object { $_ })
    if (-not ($entries | Where-Object { $_.TrimEnd('\') -eq $target.TrimEnd('\') })) {
        $path.Key.SetValue('Path', ((@($entries) + $target) -join ';'), $path.Kind)
        Write-Host "  put on your Path; open a new terminal to get greycard by name"
        Send-SettingChange
    }
    $path.Key.Close()
}

& cmd.exe /c "`"$(Join-Path $target 'register.cmd')`""
Send-AssocChanged

Write-Host ""
Write-Host "done. greycard is in the Start menu; greycard-ui is the editor and"
Write-Host "greycard the command line."

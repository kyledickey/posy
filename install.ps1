# Installs posy on Windows: a little 3D bouquet for your terminal.
#
#   irm https://raw.githubusercontent.com/kyledickey/posy/main/install.ps1 | iex
#
# It downloads the ready-made program for your computer from GitHub, checks
# it arrived intact, puts it in %LOCALAPPDATA%\Programs\posy and adds that
# folder to your PATH. Things you can set first:
#
#   $env:POSY_VERSION = "0.2.0"       install that version instead of the newest
#   $env:POSY_INSTALL_DIR = "C:\path" put it somewhere else
#   $env:POSY_DOWNLOAD_URL = "url"    fetch the files from here (a folder works too)
#
# Works in Windows PowerShell 5.1 and PowerShell 7. It only uses plain
# ASCII so it reads the same however it was saved.

function Install-Posy {
    $ErrorActionPreference = 'Stop'
    # The progress bar makes downloads in Windows PowerShell very slow.
    $ProgressPreference = 'SilentlyContinue'

    $repo = 'kyledickey/posy'
    $version = $env:POSY_VERSION
    $installDir = $env:POSY_INSTALL_DIR
    if (-not $installDir) {
        $installDir = Join-Path $env:LOCALAPPDATA 'Programs\posy'
    }
    $heart = [string][char]0x2665

    function Step($label, $detail) {
        Write-Host '  ' -NoNewline
        Write-Host $heart -ForegroundColor Magenta -NoNewline
        Write-Host " $label " -NoNewline
        Write-Host $detail -ForegroundColor DarkGray
    }

    Write-Host ''
    Write-Host '  posy' -ForegroundColor Magenta -NoNewline
    Write-Host ': a little bouquet for your terminal' -ForegroundColor DarkGray
    Write-Host ''

    # ----- what kind of computer is this -----

    # A 32-bit PowerShell on 64-bit Windows reports the real kind here.
    $cpu = $env:PROCESSOR_ARCHITEW6432
    if (-not $cpu) { $cpu = $env:PROCESSOR_ARCHITECTURE }
    switch ($cpu) {
        'AMD64' { $target = 'x86_64-pc-windows-msvc'; $kind = 'x86_64' }
        'ARM64' { $target = 'aarch64-pc-windows-msvc'; $kind = 'arm64' }
        default {
            throw "there is no ready-made posy for $cpu Windows yet.`n" +
                "    With Rust installed you can build it: cargo install --git https://github.com/$repo"
        }
    }
    Step 'this computer' "windows $kind"

    # ----- getting files from a release -----

    try {
        [Net.ServicePointManager]::SecurityProtocol =
            [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    } catch {}

    $work = Join-Path ([IO.Path]::GetTempPath()) ("posy-install-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        function Get-ReleaseFile($name) {
            $out = Join-Path $work $name
            $base = $env:POSY_DOWNLOAD_URL
            if ($base -and $base -notmatch '^https?://') {
                # A folder on this computer, for trying out a release.
                Copy-Item (Join-Path ($base -replace '^file:///?', '') $name) $out
                return
            }
            if ($base) {
                $url = $base.TrimEnd('/') + "/$name"
            } elseif ($version) {
                $url = "https://github.com/$repo/releases/download/v$version/$name"
            } else {
                $url = "https://github.com/$repo/releases/latest/download/$name"
            }
            try {
                Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $out
                return
            } catch {}
            # A private repository: use the GitHub CLI, if it is signed in.
            if (Get-Command gh -ErrorAction SilentlyContinue) {
                $tag = @()
                if ($version) { $tag = @("v$version") }
                try {
                    & gh release download @tag --repo $repo --pattern $name --clobber --dir $work 2>$null
                } catch {}
                if ($LASTEXITCODE -eq 0 -and (Test-Path $out)) { return }
            }
            throw "couldn't download posy from github.com/$repo.`n" +
                "    Check your connection, or that version $(if ($version) { $version } else { '(newest)' }) exists."
        }

        $archive = "posy-$target.zip"
        Write-Host '  ... downloading posy' -ForegroundColor DarkGray
        Get-ReleaseFile $archive
        Get-ReleaseFile 'checksums.txt'
        Step 'downloaded' $archive

        $want = $null
        foreach ($line in Get-Content (Join-Path $work 'checksums.txt')) {
            $parts = $line.Trim() -split '\s+', 2
            if ($parts.Count -eq 2 -and $parts[1].TrimStart('*') -eq $archive) {
                $want = $parts[0].ToLower()
            }
        }
        $have = (Get-FileHash -Algorithm SHA256 (Join-Path $work $archive)).Hash.ToLower()
        if (-not $want -or $want -ne $have) {
            throw "the download doesn't match its checksum, so nothing was installed."
        }
        Step 'checked' 'it arrived intact'

        $unpacked = Join-Path $work 'unpacked'
        Expand-Archive -Path (Join-Path $work $archive) -DestinationPath $unpacked -Force
        $fresh = Join-Path $unpacked 'posy.exe'
        if (-not (Test-Path $fresh)) { throw "the download didn't have posy in it." }

        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
        $exe = Join-Path $installDir 'posy.exe'
        $staged = Join-Path $installDir '.posy-new.exe'
        $old = Join-Path $installDir '.posy-old.exe'
        Copy-Item $fresh $staged -Force
        # A running posy can't be replaced, but it can be moved aside.
        Remove-Item $old -Force -ErrorAction SilentlyContinue
        if (Test-Path $exe) { Move-Item $exe $old -Force }
        Move-Item $staged $exe -Force
        Remove-Item $old -Force -ErrorAction SilentlyContinue
        $installed = 'posy'
        try { $installed = (& $exe --version) } catch {}
        Step 'installed' $installed
    } finally {
        Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    }

    # ----- somewhere Windows looks for programs -----

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $folders = @($userPath -split ';' | Where-Object { $_ })
    if ($folders -notcontains $installDir) {
        [Environment]::SetEnvironmentVariable('Path', (($folders + $installDir) -join ';'), 'User')
        Step 'added to your PATH' $installDir
    }
    # And for this window too, so it works straight away.
    if (@($env:Path -split ';') -notcontains $installDir) {
        $env:Path = $env:Path.TrimEnd(';') + ';' + $installDir
    }

    Write-Host ''
    Write-Host "  all done $heart" -ForegroundColor Magenta
    Write-Host ''
    Write-Host '    posy          ' -ForegroundColor Magenta -NoNewline
    Write-Host "see today's bouquet"
    Write-Host '    posy -b roses ' -ForegroundColor Magenta -NoNewline
    Write-Host 'or a dozen roses'
    Write-Host '    posy update   ' -ForegroundColor Magenta -NoNewline
    Write-Host 'get the newest version later'
    Write-Host ''
    Write-Host '  It looks best in Windows Terminal.' -ForegroundColor DarkGray
    Write-Host ''
}

try {
    Install-Posy
} catch {
    Write-Host ''
    Write-Host "  error: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host ''
    # A script file can report failure; a piped `irm | iex` must not close
    # the window it runs in.
    if ($MyInvocation.MyCommand.Path) { exit 1 }
}

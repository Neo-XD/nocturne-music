$ErrorActionPreference = "Stop"

$workspace = Get-Location
$mpvDir = Join-Path $workspace ".libmpv"
$srcTauriDir = Join-Path $workspace "src-tauri"

Write-Host "Setting up libmpv in $mpvDir..."
New-Item -ItemType Directory -Force -Path $mpvDir | Out-Null
New-Item -ItemType Directory -Force -Path $srcTauriDir | Out-Null

$zipPath = Join-Path $mpvDir "mpv-dev.7z"
$url = "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20261010/mpv-dev-x86_64-20261010-git-b2c255c13e.7z"

if (-not (Test-Path (Join-Path $mpvDir "libmpv-2.dll"))) {
    Write-Host "Downloading mpv dev package from $url..."
    & curl.exe -fsSL -o $zipPath $url
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $zipPath)) {
        Write-Host "Pinned URL returned error or was pruned. Fetching latest release from GitHub API..."
        $headers = @{ "User-Agent" = "nocturne-build" }
        $rel = Invoke-RestMethod -Uri "https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest" -Headers $headers
        $asset = $rel.assets | Where-Object { $_.name -like "mpv-dev-x86_64-*.7z" -and $_.name -notlike "*v3*" } | Select-Object -First 1
        if (-not $asset) { throw "Could not find baseline mpv-dev-x86_64 asset in latest release" }
        $url = $asset.browser_download_url
        Write-Host "Downloading from latest release: $url"
        & curl.exe -fsSL -o $zipPath $url
        if ($LASTEXITCODE -ne 0) { throw "Failed to download $url" }
    }
    Write-Host "Extracting..."
    tar.exe -xf $zipPath -C $mpvDir
    Remove-Item $zipPath -ErrorAction SilentlyContinue
}

$dllPath = Join-Path $mpvDir "libmpv-2.dll"
if (-not (Test-Path $dllPath)) {
    throw "libmpv-2.dll not found in $mpvDir"
}

# Copy DLL to src-tauri so Tauri bundle includes it
Copy-Item $dllPath (Join-Path $srcTauriDir "libmpv-2.dll") -Force
Write-Host "Copied libmpv-2.dll to src-tauri/libmpv-2.dll"

# Locate MSVC toolchain for dumpbin and lib.exe
$msvcBin = ""
$dumpbin = Get-Command "dumpbin.exe" -ErrorAction SilentlyContinue
$libExe = Get-Command "lib.exe" -ErrorAction SilentlyContinue

if ($dumpbin -and $libExe) {
    $dumpbinPath = $dumpbin.Source
    $libPath = $libExe.Source
} else {
    $msvcTools = Get-ChildItem "C:\Program Files (x86)\Microsoft Visual Studio" -Recurse -Filter "dumpbin.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($msvcTools) {
        $msvcBin = $msvcTools.DirectoryName
        $dumpbinPath = Join-Path $msvcBin "dumpbin.exe"
        $libPath = Join-Path $msvcBin "lib.exe"
    } else {
        throw "Could not find Visual Studio MSVC tools (dumpbin.exe / lib.exe)"
    }
}

Write-Host "Using dumpbin at: $dumpbinPath"
Write-Host "Using lib at: $libPath"

# Extract exports and generate mpv.def
Set-Location $mpvDir
$exports = & $dumpbinPath /exports libmpv-2.dll |
    Select-String -Pattern '^\s+\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]+\s+(\w+)' |
    ForEach-Object { $_.Matches[0].Groups[1].Value }

if ($exports.Count -lt 50) {
    throw "Parsed only $($exports.Count) exports from libmpv-2.dll"
}

@("EXPORTS") + ($exports | ForEach-Object { "    $_" }) | Set-Content -Path "mpv.def" -Encoding ascii
& $libPath /def:mpv.def /name:libmpv-2.dll /out:mpv.lib /machine:x64

if (-not (Test-Path (Join-Path $mpvDir "mpv.lib"))) {
    throw "Failed to generate mpv.lib"
}

Write-Host "Successfully generated mpv.lib ($($exports.Count) exports)"

# Ensure .cargo/config.toml exists so cargo automatically finds mpv.lib
$cargoDir = Join-Path $workspace ".cargo"
New-Item -ItemType Directory -Force -Path $cargoDir | Out-Null
$cargoConfig = Join-Path $cargoDir "config.toml"

$escapedMpvDir = $mpvDir -replace '\\', '/'
$configContent = @"
[build]
rustflags = ["-L", "native=$escapedMpvDir"]
"@

Set-Content -Path $cargoConfig -Value $configContent -Encoding utf8
Write-Host "Configured .cargo/config.toml with rustflags"

Set-Location $workspace
Write-Host "Setup completed successfully!"

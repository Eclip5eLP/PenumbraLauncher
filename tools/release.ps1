<#
.SYNOPSIS
  Builds, signs and publishes a launcher release (docs/09-self-update-and-release.md).

.DESCRIPTION
  1. checks the git tree is clean, bumps the version (tauri.conf.json, Cargo.toml, package.json)
  2. runs the tests (skip with -SkipTests)
  3. builds the NSIS installer with the updater signature (key from C:\Eclip5eDev\keys)
  4. stages installer + .sig + latest.json (stable also refreshes latest-beta.json) under out/stage/launcher
  5. commits the version bump, tags launcher-v<version>, uploads with penumbra-pack (installer before latest.json)

.EXAMPLE
  ./tools/release.ps1 -Version 0.2.0 -Notes "Cloud saves"
  ./tools/release.ps1 -Version 0.2.1-beta.1 -Channel beta
#>
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [ValidateSet("stable", "beta")][string]$Channel = "stable",
    [string]$Notes = "",
    [switch]$SkipTests,
    [switch]$NoUpload,
    [switch]$AllowDirty,
    [string]$KeyDir = "C:\Eclip5eDev\keys"
)
$ErrorActionPreference = "Stop"
$root = (git rev-parse --show-toplevel).Trim()
Set-Location $root
$app = Join-Path $root "apps\launcher"
$utf8 = New-Object System.Text.UTF8Encoding($false)

function Step($msg) { Write-Host "`n== $msg" -ForegroundColor Cyan }
function Run($exe, [string[]]$argv) {
    # Windows PowerShell turns a tool's stderr progress output into errors under "Stop": relax it while the tool
    # runs and judge success by the exit code only.
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { & $exe @argv 2>&1 | ForEach-Object { "$_" } } finally { $ErrorActionPreference = $prev }
    if ($LASTEXITCODE -ne 0) { throw "$exe $($argv -join ' ') failed ($LASTEXITCODE)" }
}
function ReplaceFirst($path, $pattern, $replacement) {
    $text = [IO.File]::ReadAllText($path)
    $re = [regex]$pattern
    if (-not $re.IsMatch($text)) { throw "version not found in $path" }
    [IO.File]::WriteAllText($path, $re.Replace($text, $replacement, 1), $utf8)
}

if ($Version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$') { throw "Version must be SemVer, e.g. 0.2.0 or 0.2.1-beta.1" }
if ($Channel -eq "stable" -and $Version -match '-') { throw "Pre-release versions go to -Channel beta" }
if (-not $AllowDirty -and (git status --porcelain)) { throw "Working tree not clean. Commit first (or -AllowDirty)." }
if ((git rev-parse --abbrev-ref HEAD).Trim() -ne "dev") { throw "Release from the dev branch." }
foreach ($f in "updater.key", "updater.key.password") {
    if (-not (Test-Path (Join-Path $KeyDir $f))) { throw "missing $KeyDir\$f" }
}

Step "Version $Version ($Channel)"
ReplaceFirst "$app\src-tauri\tauri.conf.json" '"version":\s*"[^"]+"' "`"version`": `"$Version`""
ReplaceFirst "$app\src-tauri\Cargo.toml" '(?m)^version\s*=\s*"[^"]+"' "version = `"$Version`""
ReplaceFirst "$app\package.json" '"version":\s*"[^"]+"' "`"version`": `"$Version`""

if (-not $SkipTests) {
    Step "Tests"
    Run cargo @("test", "--workspace", "--quiet")
    Push-Location $app; try { Run npm @("run", "check") } finally { Pop-Location }
}

Step "Build signed installer"
$env:TAURI_SIGNING_PRIVATE_KEY = [IO.File]::ReadAllText((Join-Path $KeyDir "updater.key")).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [IO.File]::ReadAllText((Join-Path $KeyDir "updater.key.password")).Trim()
try {
    Push-Location $app
    try { Run npx @("tauri", "build", "--bundles", "nsis") } finally { Pop-Location }
} finally {
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}
$nsis = Join-Path $root "target\release\bundle\nsis"
$setup = Get-ChildItem $nsis -Filter "*_${Version}_x64-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "installer for $Version not found in $nsis" }
$sig = "$($setup.FullName).sig"
if (-not (Test-Path $sig)) { throw "updater signature missing (is createUpdaterArtifacts on?)" }

Step "Stage"
$name = "PenumbraLauncher_${Version}_x64-setup.exe"
$stage = Join-Path $root "out\stage\launcher"
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $setup.FullName (Join-Path $stage $name) -Force
Copy-Item $sig (Join-Path $stage "$name.sig") -Force
$base = "https://eclip5e.com/penumbra/launcher/"
$manifest = [ordered]@{
    version   = $Version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{ signature = [IO.File]::ReadAllText($sig).Trim(); url = "$base$name" }
    }
    installer = $name
    size      = $setup.Length
}
$json = $manifest | ConvertTo-Json -Depth 5
$targets = if ($Channel -eq "stable") { @("latest.json", "latest-beta.json") } else { @("latest-beta.json") }
foreach ($t in $targets) { [IO.File]::WriteAllText((Join-Path $stage $t), $json, $utf8) }
Write-Host ("installer {0:N1} MB, manifests: {1}" -f ($setup.Length / 1MB), ($targets -join ", "))

Step "Commit + tag"
git add -A "$app\src-tauri\tauri.conf.json" "$app\src-tauri\Cargo.toml" "$app\package.json" Cargo.lock
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) {
    git commit -q -m "Release launcher $Version" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
    if ($LASTEXITCODE -ne 0) { throw "commit failed" }
}
git tag -f "launcher-v$Version" | Out-Null

if ($NoUpload) { Write-Host "`nStaged only (-NoUpload). Ship with: penumbra-pack upload"; exit 0 }
Step "Upload"
$pack = Join-Path $root "target\release\penumbra-pack.exe"
if (-not (Test-Path $pack)) { Run cargo @("build", "--release", "-p", "penumbra-pack") }
Run $pack @("upload")
Write-Host "`nReleased $Version -> $base$name" -ForegroundColor Green

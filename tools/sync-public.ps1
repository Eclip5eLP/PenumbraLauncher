<#
.SYNOPSIS
  Builds a new commit on the public `main` branch from the current `dev` tree, minus the paths in .publicignore.

.DESCRIPTION
  `main` never shares history with `dev`: each sync is a snapshot commit whose parent is the previous `main`
  commit. Private files therefore can't leak through old commits. Nothing is pushed; push `main` yourself
  (or pass -Push) after checking it.

.EXAMPLE
  ./tools/sync-public.ps1 -Message "Download engine and install flow"
  ./tools/sync-public.ps1 -Push
#>
param(
    [string]$Source = "dev",
    [string]$Target = "main",
    [string]$Message = "",
    [string]$Remote = "public",
    [switch]$Push
)
$ErrorActionPreference = "Stop"
Set-Location (git rev-parse --show-toplevel)

$srcSha = (git rev-parse --verify "$Source^{commit}").Trim()
$hasTarget = $true
git rev-parse --verify --quiet "refs/heads/$Target" *> $null
if ($LASTEXITCODE -ne 0) { $hasTarget = $false }

$ignore = Get-Content .publicignore | Where-Object { $_ -and -not $_.StartsWith("#") } | ForEach-Object { $_.Trim() }

# Build the filtered tree in a throwaway index so the working tree and the real index stay untouched.
$tmpIndex = Join-Path ([IO.Path]::GetTempPath()) ("pgl-public-index-" + [guid]::NewGuid())
$env:GIT_INDEX_FILE = $tmpIndex
try {
    git read-tree $srcSha
    foreach ($p in $ignore) { git rm -r --cached --quiet --ignore-unmatch -- $p | Out-Null }
    $tree = (git write-tree).Trim()
} finally {
    Remove-Item Env:GIT_INDEX_FILE
    Remove-Item $tmpIndex -ErrorAction SilentlyContinue
}

# Safety net: refuse if anything private slipped through.
$leaks = git ls-tree -r --name-only $tree | Where-Object {
    $f = $_; $ignore | Where-Object { $f -eq $_ -or $f.StartsWith("$_/") }
}
if ($leaks) { throw "Private paths still in tree: $($leaks -join ', ')" }

if ($hasTarget -and ((git rev-parse "$Target^{tree}").Trim() -eq $tree)) {
    Write-Host "main is already up to date with $Source ($($srcSha.Substring(0,8)))."
    exit 0
}

$subject = if ($Message) { $Message } else { "Sync from $Source" }
$body = "$subject`n`nSource: $Source @ $($srcSha.Substring(0,12))"
$msgFile = New-TemporaryFile
[IO.File]::WriteAllText($msgFile.FullName, $body, (New-Object System.Text.UTF8Encoding($false)))

if ($hasTarget) {
    $commit = (git commit-tree $tree -p $Target -F $msgFile).Trim()
} else {
    $commit = (git commit-tree $tree -F $msgFile).Trim()
}
Remove-Item $msgFile
git update-ref "refs/heads/$Target" $commit
Write-Host "main -> $($commit.Substring(0,8)) (from $Source $($srcSha.Substring(0,8)))"

if ($Push) { git push $Remote "$($Target):main" }

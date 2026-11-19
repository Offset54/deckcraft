<#
.SYNOPSIS
    Downloads the SlideCraft .pptx round-trip corpus into a private folder and checks each file.

.DESCRIPTION
    Reads manifest.json (next to this script), downloads every deck to -Destination, and checks
    each file's SHA-256 against the manifest. A file that fails the check is deleted, never kept.
    Prints which decks failed. Nothing downloaded is opened or run: the files are only written
    and hashed. The decks are third-party files; keep them out of the repository (AGENTS.md 1.1).

.EXAMPLE
    .\docs\pptx-corpus\fetch.ps1 -Destination C:\corpus\pptx
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Destination,

    # Re-download files that are already present with the right hash.
    [switch] $Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'  # Invoke-WebRequest is very slow with the progress bar on
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$manifestPath = Join-Path $PSScriptRoot 'manifest.json'
$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
$dest = (Resolve-Path -LiteralPath $Destination).Path
$sep = [IO.Path]::DirectorySeparatorChar
if (($dest.TrimEnd($sep) + $sep).StartsWith($repo.TrimEnd($sep) + $sep, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Destination $dest is inside the repository. Choose a folder outside it: the decks must never be committed."
}

$failed = @()
$ok = 0
foreach ($deck in $manifest.decks) {
    $file = Join-Path $dest $deck.file
    $want = $deck.sha256.ToLowerInvariant()
    if ((Test-Path -LiteralPath $file) -and -not $Force) {
        $have = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($have -eq $want) {
            Write-Host "ok (already there)  $($deck.file)"
            $ok++
            continue
        }
    }
    $tmp = "$file.part"
    try {
        Invoke-WebRequest -Uri $deck.direct_url -OutFile $tmp -UseBasicParsing -MaximumRedirection 5 -TimeoutSec 600
        $have = (Get-FileHash -LiteralPath $tmp -Algorithm SHA256).Hash.ToLowerInvariant()
        $size = (Get-Item -LiteralPath $tmp).Length
        if ($have -ne $want) {
            Remove-Item -LiteralPath $tmp -Force
            $failed += "$($deck.file): SHA-256 mismatch (got $have, $size bytes; want $want, $($deck.bytes) bytes)"
            continue
        }
        Move-Item -LiteralPath $tmp -Destination $file -Force
        Write-Host "ok                  $($deck.file)"
        $ok++
    }
    catch {
        if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Force }
        $failed += "$($deck.file): download failed: $($_.Exception.Message)"
    }
}

Write-Host ''
Write-Host "$ok of $($manifest.decks.Count) decks downloaded and checked into $dest"
if ($failed.Count -gt 0) {
    Write-Host "$($failed.Count) failed:" -ForegroundColor Red
    $failed | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
    exit 1
}

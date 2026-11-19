# Writes dist\release-notes-vX.Y.Z.md: the CHANGELOG section of that version plus the exe's SHA-256.
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Out
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$lines = Get-Content -Encoding UTF8 (Join-Path $root 'CHANGELOG.md')
$body = @()
$inside = $false
foreach ($l in $lines) {
    if ($l -match '^## \[(.+?)\]') {
        if ($inside) { break }
        if ($Matches[1] -eq $Version) { $inside = $true; continue }
    }
    elseif ($inside) { $body += $l }
}
if (-not $inside) { Write-Warning "CHANGELOG.md has no section [$Version]; the notes will only contain the hash." }
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Exe).Hash.ToLower()
$name = Split-Path -Leaf $Exe
$text = @()
$text += ($body -join "`n").Trim()
$text += ''
$text += '### SHA-256'
$text += '```'
$text += "$hash  $name"
$text += '```'
[IO.File]::WriteAllText($Out, ($text -join "`n") + "`n", (New-Object Text.UTF8Encoding($false)))
Set-Content -Encoding ASCII -LiteralPath "$Exe.sha256" -Value "$hash  $name"
Write-Host "SHA-256: $hash"

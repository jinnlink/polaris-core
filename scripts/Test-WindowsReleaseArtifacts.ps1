[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ArtifactDirectory,
    [switch]$RequireSignature
)

$ErrorActionPreference = 'Stop'
$hashPath = Join-Path $ArtifactDirectory 'SHA256SUMS.txt'
if (-not (Test-Path -LiteralPath $hashPath)) { throw "Missing hash manifest: $hashPath" }
foreach ($line in Get-Content -LiteralPath $hashPath) {
    if ($line -notmatch '^([0-9a-fA-F]{64})  (.+)$') { throw "Invalid hash manifest line: $line" }
    $path = Join-Path $ArtifactDirectory $Matches[2]
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing release artifact: $path" }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash
    if ($actual -ne $Matches[1]) { throw "SHA-256 mismatch: $($Matches[2])" }
}

if ($RequireSignature) {
    $manifestPath = Join-Path $ArtifactDirectory 'latest.json'
    if (-not (Test-Path -LiteralPath $manifestPath)) { throw 'Signed release requires latest.json.' }
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    $platform = $manifest.platforms.'windows-x86_64'
    if (-not $platform -or [string]::IsNullOrWhiteSpace($platform.signature)) { throw 'Signed manifest has no windows-x86_64 signature.' }
    if (-not ([uri]$platform.url).Scheme.Equals('https')) { throw 'Updater artifact URL must use HTTPS.' }
    $installer = Get-ChildItem -LiteralPath $ArtifactDirectory -File -Filter '*-setup.exe' | Select-Object -First 1
    if (-not $installer) { throw 'Signed release has no NSIS installer.' }
    $signaturePath = "$($installer.FullName).sig"
    if (-not (Test-Path -LiteralPath $signaturePath)) { throw 'Signed release has no detached updater signature.' }
    if ((Get-Content -Raw -LiteralPath $signaturePath).Trim() -ne $platform.signature.Trim()) { throw 'Manifest signature does not match the detached signature.' }
}
Write-Host "Release artifact verification passed: $ArtifactDirectory"

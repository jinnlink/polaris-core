[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BundleDirectory,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][uri]$AssetBaseUrl,
    [string]$Notes = 'See the release notes for details.',
    [string]$OutputDirectory = $BundleDirectory
)

$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^v?\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$') { throw "Version is not SemVer: $Version" }
if ($AssetBaseUrl.Scheme -ne 'https') { throw 'Release assets must use HTTPS.' }

$installer = Get-ChildItem -LiteralPath $BundleDirectory -File -Filter '*-setup.exe' | Select-Object -First 1
if (-not $installer) { throw "NSIS installer was not found in $BundleDirectory" }
$signaturePath = "$($installer.FullName).sig"
if (-not (Test-Path -LiteralPath $signaturePath)) { throw "Updater signature was not found: $signaturePath" }
$signature = (Get-Content -Raw -LiteralPath $signaturePath).Trim()
if ([string]::IsNullOrWhiteSpace($signature)) { throw 'Updater signature is empty.' }

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$assetName = [uri]::EscapeDataString($installer.Name)
$manifest = [ordered]@{
    version = $Version.TrimStart('v')
    notes = $Notes
    pub_date = [DateTimeOffset]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = $signature
            url = ([uri]::new($AssetBaseUrl, $assetName)).AbsoluteUri
        }
    }
}
$manifestPath = Join-Path $OutputDirectory 'latest.json'
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM

$artifacts = @($installer.FullName, $signaturePath, $manifestPath)
$hashPath = Join-Path $OutputDirectory 'SHA256SUMS.txt'
$lines = foreach ($path in $artifacts) {
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($path))"
}
$lines | Set-Content -LiteralPath $hashPath -Encoding ascii
Write-Host "Generated latest.json and SHA256SUMS.txt for $($installer.Name)"

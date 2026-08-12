[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$PublicKey,
    [Parameter(Mandatory = $true)][uri]$Endpoint,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($PublicKey)) { throw 'Updater public key is required.' }
if ($Endpoint.Scheme -ne 'https') { throw 'Production updater endpoint must use HTTPS.' }

$config = [ordered]@{
    bundle = [ordered]@{ createUpdaterArtifacts = $true }
    plugins = [ordered]@{
        updater = [ordered]@{
            pubkey = $PublicKey.Trim()
            endpoints = @($Endpoint.AbsoluteUri)
            windows = [ordered]@{ installMode = 'passive' }
        }
    }
}
$parent = Split-Path -Parent $OutputPath
if ($parent) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
$config | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
Write-Host "Generated signed-updater release config at $OutputPath"

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$InstallerPath,
    [string]$PreviousInstallerPath,
    [ValidateSet('PreserveData', 'DeleteData')][string]$UninstallMode = 'PreserveData',
    [int]$ColdStartBudgetMs = 3000,
    [switch]$ConfirmDisposableVm
)

$ErrorActionPreference = 'Stop'
if (-not $ConfirmDisposableVm) { throw 'This smoke mutates the current-user installation. Run only in a clean disposable Windows VM with -ConfirmDisposableVm.' }
if (-not (Test-Path -LiteralPath $InstallerPath)) { throw "Installer not found: $InstallerPath" }
$appData = Join-Path $env:APPDATA 'app.polaris.desktop'
if (-not ([IO.Path]::GetFullPath($appData)).StartsWith([IO.Path]::GetFullPath($env:APPDATA), [StringComparison]::OrdinalIgnoreCase)) { throw 'Resolved app data escaped APPDATA.' }

function Invoke-Installer([string]$Path) {
    $process = Start-Process -FilePath $Path -ArgumentList '/S' -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw "Installer exited with $($process.ExitCode): $Path" }
}
function Resolve-InstalledApp {
    $entry = Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
        Get-ItemProperty | Where-Object DisplayName -eq 'Polaris' | Select-Object -First 1
    if (-not $entry) { throw 'Polaris uninstall registration was not found.' }
    $candidate = Join-Path $entry.InstallLocation 'polaris-desktop.exe'
    if (-not (Test-Path -LiteralPath $candidate)) { throw "Installed executable was not found: $candidate" }
    return @{ App = $candidate; Uninstall = $entry.UninstallString.Trim('"') }
}
function Assert-ColdStart([string]$App) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $App -PassThru
    if (-not $process.WaitForInputIdle($ColdStartBudgetMs)) { Stop-Process -Id $process.Id -Force; throw "Main window was not interactive within ${ColdStartBudgetMs}ms." }
    $watch.Stop()
    if ($watch.ElapsedMilliseconds -gt $ColdStartBudgetMs) { Stop-Process -Id $process.Id -Force; throw "Cold start exceeded budget: $($watch.ElapsedMilliseconds)ms" }
    Stop-Process -Id $process.Id -Force
    Write-Host "Cold start gate passed: $($watch.ElapsedMilliseconds)ms"
}

if ($PreviousInstallerPath) {
    Invoke-Installer $PreviousInstallerPath
    $previous = Resolve-InstalledApp
    Assert-ColdStart $previous.App
    New-Item -ItemType Directory -Force -Path $appData | Out-Null
    Set-Content -LiteralPath (Join-Path $appData 'upgrade-smoke.sentinel') -Value 'preserve-me' -Encoding ascii
}

Invoke-Installer $InstallerPath
$installed = Resolve-InstalledApp
Assert-ColdStart $installed.App
$database = Join-Path $appData 'polaris.sqlite'
if (-not (Test-Path -LiteralPath $database)) { throw "First-run database was not created: $database" }
if ($PreviousInstallerPath -and -not (Test-Path -LiteralPath (Join-Path $appData 'upgrade-smoke.sentinel'))) { throw 'Existing application data did not survive the upgrade.' }

if ($UninstallMode -eq 'DeleteData') {
    Set-Content -LiteralPath (Join-Path $appData 'delete-on-uninstall.marker') -Value 'confirmed disposable VM smoke' -Encoding ascii
}
$uninstaller = Start-Process -FilePath $installed.Uninstall -ArgumentList '/S' -PassThru -Wait
if ($uninstaller.ExitCode -ne 0) { throw "Uninstaller exited with $($uninstaller.ExitCode)." }
if ($UninstallMode -eq 'PreserveData' -and -not (Test-Path -LiteralPath $database)) { throw 'Preserve-data uninstall removed the database.' }
if ($UninstallMode -eq 'DeleteData' -and (Test-Path -LiteralPath $appData)) { throw 'Delete-data uninstall retained the default application data.' }
Write-Host "Windows release smoke passed: install, first DB, upgrade preservation, $UninstallMode uninstall."

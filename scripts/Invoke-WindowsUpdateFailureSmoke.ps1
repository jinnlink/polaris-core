[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BaselineInstallerPath,
    [Parameter(Mandatory = $true)][string]$InvalidUpdatePath,
    [int]$ColdStartBudgetMs = 3000,
    [switch]$ConfirmDisposableVm
)

$ErrorActionPreference = 'Stop'
if (-not $ConfirmDisposableVm) { throw 'This smoke mutates the current-user installation. Run only in a clean disposable Windows VM with -ConfirmDisposableVm.' }
foreach ($path in @($BaselineInstallerPath, $InvalidUpdatePath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Smoke input was not found: $path" }
}

$install = Start-Process -FilePath $BaselineInstallerPath -ArgumentList '/S' -PassThru -Wait
if ($install.ExitCode -ne 0) { throw "Baseline installer exited with $($install.ExitCode)." }
$entry = Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
    Get-ItemProperty | Where-Object DisplayName -eq 'Polaris' | Select-Object -First 1
if (-not $entry) { throw 'Baseline Polaris installation was not registered.' }
$app = Join-Path $entry.InstallLocation 'polaris-desktop.exe'
$appData = Join-Path $env:APPDATA 'app.polaris.desktop'
$database = Join-Path $appData 'polaris.sqlite'
$sentinel = Join-Path $appData 'failed-update-smoke.sentinel'

$first = Start-Process -FilePath $app -PassThru
if (-not $first.WaitForInputIdle($ColdStartBudgetMs)) { Stop-Process -Id $first.Id -Force; throw 'Baseline did not become interactive.' }
Stop-Process -Id $first.Id -Force
if (-not (Test-Path -LiteralPath $database)) { throw 'Baseline database was not created.' }
Set-Content -LiteralPath $sentinel -Value 'must-survive-failed-update' -Encoding ascii
$databaseHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $database).Hash

$failureObserved = $false
try {
    $failed = Start-Process -FilePath $InvalidUpdatePath -ArgumentList '/S' -PassThru -Wait
    $failureObserved = $failed.ExitCode -ne 0
} catch {
    $failureObserved = $true
}
if (-not $failureObserved) { throw 'Invalid update unexpectedly reported success.' }
if (-not (Test-Path -LiteralPath $sentinel)) { throw 'Failed update removed user data.' }
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $database).Hash -ne $databaseHash) { throw 'Failed update changed the database.' }

$recovered = Start-Process -FilePath $app -PassThru
if (-not $recovered.WaitForInputIdle($ColdStartBudgetMs)) { Stop-Process -Id $recovered.Id -Force; throw 'Baseline did not recover after the failed update.' }
Stop-Process -Id $recovered.Id -Force
Write-Host 'Failed-update rollback smoke passed: previous binary and database remain usable.'

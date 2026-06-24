param(
    [string]$DbPath,
    [string]$ProjectPath,
    [string]$TranscriptPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$targetDir = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target"))

if (-not $DbPath) {
    $DbPath = Join-Path $repoRoot "target\p14a-real-use.sqlite"
}
if (-not $ProjectPath) {
    $ProjectPath = Join-Path $repoRoot "examples\project-manifests\rust-mastery-lab"
}
if (-not $TranscriptPath) {
    $TranscriptPath = Join-Path $repoRoot "target\p14a-real-use-transcript.txt"
}

function Resolve-SmokePath {
    param([string]$Path)

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $repoRoot $Path))
}

function Assert-UnderTarget {
    param(
        [string]$Path,
        [string]$Name
    )

    $fullPath = Resolve-SmokePath $Path
    $targetPrefix = $targetDir.TrimEnd('\') + '\'
    if (-not $fullPath.StartsWith($targetPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Name must be under $targetDir"
    }
    return $fullPath
}

$DbPath = Assert-UnderTarget $DbPath "DbPath"
$TranscriptPath = Assert-UnderTarget $TranscriptPath "TranscriptPath"
$ProjectPath = Resolve-SmokePath $ProjectPath

New-Item -ItemType Directory -Force $targetDir | Out-Null

function Reset-SmokeDatabase {
    param([string]$Path)

    foreach ($candidate in @($Path, "$Path-shm", "$Path-wal")) {
        Remove-Item -LiteralPath $candidate -ErrorAction SilentlyContinue
    }
}

function Add-TranscriptLine {
    param([string]$Line)

    Add-Content -LiteralPath $TranscriptPath -Value $Line -Encoding UTF8
}

function Run-Logged {
    param([string[]]$Arguments)

    $commandLine = "polaris " + ($Arguments -join " ")
    Add-TranscriptLine ""
    Add-TranscriptLine "> $commandLine"

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Polaris @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    foreach ($line in $output) {
        Add-TranscriptLine $line.ToString()
    }
    if ($exitCode -ne 0) {
        throw "Command failed with exit code ${exitCode}: $commandLine"
    }
    return $output
}

function Read-CaptureId {
    param([object[]]$Output)

    $line = $Output | Where-Object { $_.ToString() -match '^capture_id:\s+' } | Select-Object -First 1
    if (-not $line) {
        throw "capture_id was not found in capture output"
    }
    return ($line.ToString() -replace '^capture_id:\s+', '').Trim()
}

function Assert-OutputContains {
    param(
        [object[]]$Output,
        [string]$Pattern,
        [string]$Description
    )

    $matched = $Output | Where-Object { $_.ToString() -match $Pattern } | Select-Object -First 1
    if (-not $matched) {
        throw "Expected $Description in command output"
    }
}

Push-Location $repoRoot
try {
    Reset-SmokeDatabase $DbPath
    Remove-Item -LiteralPath $TranscriptPath -ErrorAction SilentlyContinue
    Add-TranscriptLine "# P14A real-use smoke transcript"
    Add-TranscriptLine "repo: $repoRoot"
    Add-TranscriptLine "db: $DbPath"
    Add-TranscriptLine "project: $ProjectPath"

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $buildOutput = & cargo build -p polaris-cli 2>&1
        $buildExit = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    Add-TranscriptLine ""
    Add-TranscriptLine "> cargo build -p polaris-cli"
    foreach ($line in $buildOutput) {
        Add-TranscriptLine $line.ToString()
    }
    if ($buildExit -ne 0) {
        throw "cargo build failed with exit code $buildExit"
    }

    $script:Polaris = Join-Path $repoRoot "target\debug\polaris.exe"
    if (-not (Test-Path -LiteralPath $Polaris)) {
        throw "polaris.exe was not built at $Polaris"
    }

    $initOutput = Run-Logged @("--db", $DbPath, "init", "--pack", "packs\rust")
    Assert-OutputContains $initOutput '^initialized$' "database initialization confirmation"

    $profileOutput = Run-Logged @(
        "--db", $DbPath,
        "ai-profile", "set",
        "--persona", "socratic_tutor",
        "--verbosity", "detailed",
        "--explanation-depth", "examples_first",
        "--proactivity", "stuck_only",
        "--intervention-frequency", "normal",
        "--correction-style", "guided"
    )
    Assert-OutputContains $profileOutput '^guidance:' "AI profile guidance"

    $projectOutput = Run-Logged @("project", "detect", "--path", $ProjectPath)
    Assert-OutputContains $projectOutput '^project_id:\s+rust-mastery-lab$' "project id"
    Assert-OutputContains $projectOutput '^default_pack:\s+rust$' "default pack"
    Assert-OutputContains $projectOutput '^today_command:' "today command"

    $captureOutput = Run-Logged @(
        "--db", $DbPath,
        "capture",
        "--text", "I learned that Rust ownership decides which binding is responsible for dropping a value.",
        "--source", "real-use-smoke",
        "--candidate-concept", "ownership"
    )
    $captureId = Read-CaptureId $captureOutput
    Assert-OutputContains $captureOutput '^recorded_only:\s+true$' "recorded_only capture receipt"
    Write-Host "capture_id: $captureId"

    $inboxOutput = Run-Logged @("--db", $DbPath, "inbox", "list")
    Assert-OutputContains $inboxOutput $captureId "capture id in inbox"
    Assert-OutputContains $inboxOutput 'accept' "student action choices"

    $actOutput = Run-Logged @("--db", $DbPath, "inbox", "act", "--capture", $captureId, "--action", "accept")
    Assert-OutputContains $actOutput '^status:\s+practice_ready$' "practice_ready status"

    $practiceOutput = Run-Logged @("--db", $DbPath, "inbox", "practice", "--capture", $captureId)
    Assert-OutputContains $practiceOutput '^prompt:' "inbox practice prompt"
    Assert-OutputContains $practiceOutput '^task_type:\s+explain$' "inbox practice task type"

    $submitOutput = Run-Logged @(
        "--db", $DbPath,
        "inbox", "submit",
        "--capture", $captureId,
        "--response", "Ownership means a value has one owner, and that owner is responsible for dropping it when it goes out of scope.",
        "--confidence", "4",
        "--session", "p14a-real-use"
    )
    Assert-OutputContains $submitOutput '^attempt_id:' "submitted attempt id"
    Assert-OutputContains $submitOutput '^provisional_score:\s+0\.700$' "provisional score"
    Assert-OutputContains $submitOutput '^degraded:\s+true$' "degraded scoring receipt"

    $mirrorOutput = Run-Logged @("--db", $DbPath, "learner-mirror", "--json")
    Assert-OutputContains $mirrorOutput '"confidence_curve": \[' "learner mirror confidence curve"
    Assert-OutputContains $mirrorOutput '"concept_id": "ownership"' "ownership in learner mirror"

    Write-Host "P14A real-use smoke passed."
    Write-Host "transcript: $TranscriptPath"
}
finally {
    Pop-Location
}

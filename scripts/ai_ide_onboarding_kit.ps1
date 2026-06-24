param(
    [string]$ProjectPath,
    [string]$DbPath,
    [string]$OutDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$targetDir = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target"))

if (-not $ProjectPath) {
    $ProjectPath = Join-Path $repoRoot "examples\project-manifests\rust-mastery-lab"
}
if (-not $DbPath) {
    $DbPath = Join-Path $repoRoot "target\p14c-ai-ide-kit.sqlite"
}
if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot "target\p14c-ai-ide-kit"
}

function Resolve-KitPath {
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

    $fullPath = Resolve-KitPath $Path
    $targetPrefix = $targetDir.TrimEnd('\') + '\'
    if (-not $fullPath.StartsWith($targetPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Name must be under $targetDir"
    }
    return $fullPath
}

function Test-IsReparsePoint {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return $false
    }
    $item = Get-Item -LiteralPath $Path -Force
    return (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Assert-NoReparseAncestor {
    param(
        [string]$Path,
        [string]$Name,
        [bool]$TreatAsDirectory
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if ($TreatAsDirectory) {
        $cursor = $fullPath
    }
    else {
        $cursor = Split-Path -Parent $fullPath
    }

    while ($cursor -and -not (Test-Path -LiteralPath $cursor)) {
        $cursor = Split-Path -Parent $cursor
    }

    $targetRoot = $targetDir.TrimEnd('\')
    while ($cursor) {
        $cursorFull = [System.IO.Path]::GetFullPath($cursor).TrimEnd('\')
        if (-not $cursorFull.StartsWith($targetRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "$Name resolved outside target while checking parents: $cursorFull"
        }
        if (Test-IsReparsePoint $cursorFull) {
            throw "$Name uses a reparse point inside target: $cursorFull"
        }
        if ($cursorFull.Equals($targetRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            break
        }
        $cursor = Split-Path -Parent $cursorFull
    }
}

function Invoke-Logged {
    param(
        [string]$Command,
        [string[]]$Arguments
    )

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Command @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    if ($exitCode -ne 0) {
        $text = ($output | ForEach-Object { $_.ToString() }) -join "`n"
        throw "$Command failed with exit code $exitCode`n$text"
    }
    return @($output | ForEach-Object { $_.ToString() })
}

function Reset-TargetDatabase {
    param([string]$Path)

    foreach ($candidate in @($Path, "$Path-shm", "$Path-wal")) {
        if (Test-IsReparsePoint $candidate) {
            throw "Refusing to remove reparse point: $candidate"
        }
        Remove-Item -LiteralPath $candidate -ErrorAction SilentlyContinue
    }
}

New-Item -ItemType Directory -Force $targetDir | Out-Null
if (Test-IsReparsePoint $targetDir) {
    throw "Target directory must not be a reparse point: $targetDir"
}

$DbPath = Assert-UnderTarget $DbPath "DbPath"
$OutDir = Assert-UnderTarget $OutDir "OutDir"
$ProjectPath = Resolve-KitPath $ProjectPath

if ([System.IO.Path]::GetExtension($DbPath) -ne ".sqlite") {
    throw "DbPath must end with .sqlite"
}
Assert-NoReparseAncestor $DbPath "DbPath" $false
Assert-NoReparseAncestor $OutDir "OutDir" $true

if (-not (Test-Path -LiteralPath $ProjectPath -PathType Container)) {
    throw "ProjectPath does not exist or is not a directory: $ProjectPath"
}

New-Item -ItemType Directory -Force (Split-Path -Parent $DbPath) | Out-Null
New-Item -ItemType Directory -Force $OutDir | Out-Null
Assert-NoReparseAncestor $DbPath "DbPath" $false
Assert-NoReparseAncestor $OutDir "OutDir" $true
Reset-TargetDatabase $DbPath

Push-Location $repoRoot
try {
    Invoke-Logged "cargo" @("build", "-p", "polaris-cli") | Out-Null

    $polaris = Join-Path $repoRoot "target\debug\polaris.exe"
    if (-not (Test-Path -LiteralPath $polaris)) {
        throw "polaris.exe was not built at $polaris"
    }

    Invoke-Logged $polaris @("--db", $DbPath, "init", "--pack", "packs\rust") | Out-Null
    Invoke-Logged $polaris @(
        "--db",
        $DbPath,
        "ai-profile",
        "set",
        "--persona",
        "balanced_mentor",
        "--verbosity",
        "normal",
        "--explanation-depth",
        "key_steps",
        "--proactivity",
        "stuck_only",
        "--intervention-frequency",
        "normal",
        "--correction-style",
        "guided"
    ) | Out-Null

    $detectOutput = Invoke-Logged $polaris @("project", "detect", "--path", $ProjectPath)
    $detectText = $detectOutput -join "`n"
    $projectIdMatch = [regex]::Match($detectText, '(?m)^project_id:\s*(.+)$')
    if (-not $projectIdMatch.Success) {
        throw "project detect output did not include project_id"
    }
    $projectId = $projectIdMatch.Groups[1].Value.Trim()

    $configPath = Join-Path $OutDir "mcp-config.json"
    $promptPath = Join-Path $OutDir "start-learning-prompt.md"
    $checklistPath = Join-Path $OutDir "checklist.md"

    $config = [ordered]@{
        mcpServers = [ordered]@{
            "polaris-core" = [ordered]@{
                command = $polaris
                args = @("--db", $DbPath, "mcp")
                cwd = $ProjectPath
            }
        }
    }
    $config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $configPath -Encoding UTF8

    $prompt = @(
        "You are my learning assistant. Connect to Polaris first, then guide me through the current course repository.",
        "",
        "Startup steps:",
        "1. Call Polaris MCP detect_project_manifest to confirm the current course project. If cwd does not resolve, pass path=`"$ProjectPath`".",
        "2. Call get_ai_interaction_profile and follow its guidance for persona, verbosity, explanation depth, proactivity, and intervention frequency.",
        "3. Start from the course repository's own today entry. The course drives teaching content; Polaris owns learning state, evidence, scheduling, and mirror state.",
        "",
        "During learning:",
        "- When I paste material, notes, error logs, code snippets, or chat excerpts, save them with capture_evidence. This is raw capture only; it does not mean I mastered it.",
        "- Periodically call list_learner_inbox for saved but unprocessed material, and offer only 2 to 3 natural choices.",
        "- If I want to practice an inbox item, first call act_on_learner_inbox_item(action=accept), then call draft_inbox_practice.",
        "- After I answer an inbox practice item, ask for or record my confidence, then call submit_inbox_practice with my answer and confidence.",
        "- For ordinary course exercises or non-inbox questions you create, first call get_next_task or use the course's explicit concept, collect my confidence, then call submit_evidence with session, concept_id or concept, response, and confidence.",
        "- Do not treat your own score, judgement, or encouragement as mastery authority. Mastery can only be updated by the Polaris engine from evidence.",
        "- When you need my current learning state, call get_learner_mirror. When you need a local scheduling hint, use get_next_task, but keep course explanations grounded in the current repository."
    ) -join [Environment]::NewLine
    $prompt | Set-Content -LiteralPath $promptPath -Encoding UTF8

    $checklist = @(
        "# Polaris AI IDE onboarding checklist",
        "",
        "## Generated paths",
        "",
        "- project_id: $projectId",
        "- project_path: $ProjectPath",
        "- command: $polaris",
        "- db: $DbPath",
        "- config: $configPath",
        "- prompt: $promptPath",
        "- checklist: $checklistPath",
        "",
        "## Setup steps",
        "",
        "1. Copy the ``mcpServers.polaris-core`` block from ``mcp-config.json`` into your AI IDE MCP configuration.",
        "2. Open the course repository in the AI IDE: ``$ProjectPath``.",
        "3. Restart or refresh the AI IDE MCP server list.",
        "4. Paste ``start-learning-prompt.md`` into the AI conversation.",
        "5. Ask the AI to call ``detect_project_manifest`` first. You should see ``project_id: $projectId``.",
        "6. Ask the AI to call ``get_ai_interaction_profile`` and follow ``guidance``.",
        "7. Paste a learning note and ask the AI to save it with ``capture_evidence``. Confirm ``recorded_only=true``.",
        "8. Ask the AI to call ``list_learner_inbox``, then ``act_on_learner_inbox_item(action=accept)`` and ``draft_inbox_practice``.",
        "9. Answer the generated practice item, then ask the AI to call ``submit_inbox_practice`` with your answer and confidence.",
        "10. Ask the AI to call ``get_learner_mirror`` and confirm this attempt appears in the mirror.",
        "",
        "## Troubleshooting",
        "",
        "- If the AI cannot find the project manifest, ask it to call ``detect_project_manifest`` with ``path=`"$ProjectPath`"``.",
        "- If the AI IDE does not support ``cwd``, keep ``command`` and ``args`` and require the AI to pass the project path explicitly.",
        "- For a real long-lived database, change the value after ``--db`` to ``C:\MyProject\polaris-data\polaris.sqlite`` after initializing that database.",
        "- You do not need one MCP per course. A course repository only needs ``p-os.toml``; the same Polaris MCP can serve it."
    ) -join [Environment]::NewLine
    $checklist | Set-Content -LiteralPath $checklistPath -Encoding UTF8

    Write-Host "P14C AI IDE onboarding kit generated."
    Write-Host "project_id: $projectId"
    Write-Host "command: $polaris"
    Write-Host "db: $DbPath"
    Write-Host "cwd: $ProjectPath"
    Write-Host "config: $configPath"
    Write-Host "prompt: $promptPath"
    Write-Host "checklist: $checklistPath"
}
finally {
    Pop-Location
}

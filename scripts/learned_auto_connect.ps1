param(
    [string]$LearnedRoot,
    [string]$DbPath,
    [string]$OutDir,
    [int]$MaxDepth = 3
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$targetDir = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target"))

if (-not $LearnedRoot) {
    $LearnedRoot = "C:\MyProject\Learned"
}
if (-not $DbPath) {
    $DbPath = Join-Path $repoRoot "target\p14d-learned-auto.sqlite"
}
if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot "target\p14d-learned-auto-connect"
}
if ($MaxDepth -lt 0) {
    throw "MaxDepth must be greater than or equal to 0"
}

function Resolve-KitPath {
    param([string]$Path)

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $repoRoot $Path))
}

function Resolve-InputPath {
    param([string]$Path)

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $Path))
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

$LearnedRoot = Resolve-InputPath $LearnedRoot
$DbPath = Assert-UnderTarget $DbPath "DbPath"
$OutDir = Assert-UnderTarget $OutDir "OutDir"

if ([System.IO.Path]::GetExtension($DbPath) -ne ".sqlite") {
    throw "DbPath must end with .sqlite"
}
Assert-NoReparseAncestor $DbPath "DbPath" $false
Assert-NoReparseAncestor $OutDir "OutDir" $true

if (-not (Test-Path -LiteralPath $LearnedRoot -PathType Container)) {
    throw "LearnedRoot does not exist or is not a directory: $LearnedRoot"
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

    $scanOutput = Invoke-Logged $polaris @(
        "project",
        "scan",
        "--root",
        $LearnedRoot,
        "--max-depth",
        $MaxDepth.ToString(),
        "--json"
    )
    $scanJson = $scanOutput -join "`n"
    $scan = $scanJson | ConvertFrom-Json
    $projects = @($scan.projects)
    if ($projects.Count -eq 0) {
        throw "No p-os.toml learning project was found under $LearnedRoot"
    }
    $defaultProject = $projects | Select-Object -First 1
    $defaultProjectId = $defaultProject.manifest.project_id
    $defaultProjectRoot = $defaultProject.project_root

    $configPath = Join-Path $OutDir "mcp-config.json"
    $promptPath = Join-Path $OutDir "start-from-learned-prompt.md"
    $checklistPath = Join-Path $OutDir "checklist.md"
    $projectsPath = Join-Path $OutDir "projects.json"

    $scanJson | Set-Content -LiteralPath $projectsPath -Encoding UTF8

    $config = [ordered]@{
        mcpServers = [ordered]@{
            "polaris-core" = [ordered]@{
                command = $polaris
                args = @("--db", $DbPath, "mcp")
                cwd = $LearnedRoot
            }
        }
    }
    $config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $configPath -Encoding UTF8

    $prompt = @(
        "You are my learning assistant. The AI IDE is opened at a learning root, not necessarily a single course repository.",
        "",
        "Startup steps:",
        "1. Call Polaris MCP discover_learning_projects with root=`"$LearnedRoot`" and max_depth=$MaxDepth.",
        "2. If exactly one project is returned, select it. If multiple projects are returned, show me the 2 or 3 most relevant choices and ask which one to study.",
        "3. Call detect_project_manifest with path=<selected project_root> and confirm the selected course project.",
        "4. Call get_ai_interaction_profile and follow its guidance for persona, verbosity, explanation depth, proactivity, intervention frequency, and correction style.",
        "5. Start from the selected course repository's own today entry. The course drives teaching content; Polaris owns learning state, evidence, scheduling, inbox practice, and mirror state.",
        "",
        "Default project found by this setup:",
        "- project_id: $defaultProjectId",
        "- project_root: $defaultProjectRoot",
        "",
        "During learning:",
        "- Do not modify files under $LearnedRoot unless I explicitly ask for code or course edits. Use Polaris tools for learning state.",
        "- When I paste material, notes, error logs, code snippets, or chat excerpts, save them with capture_evidence. This is raw capture only; it does not mean I mastered it.",
        "- Periodically call list_learner_inbox for saved but unprocessed material, and offer only 2 to 3 natural choices.",
        "- If I want to practice an inbox item, first call act_on_learner_inbox_item(action=accept), then call draft_inbox_practice.",
        "- After I answer an inbox practice item, ask for or record my confidence, then call submit_inbox_practice with my answer and confidence.",
        "- For ordinary course exercises or non-inbox questions you create, first call get_next_task or use the course's explicit concept, collect my confidence, then call submit_evidence with session, concept_id or concept, response, and confidence.",
        "- Do not treat your own score, judgement, or encouragement as mastery authority. Mastery can only be updated by the Polaris engine from evidence.",
        "- When you need my current learning state, call get_learner_mirror. When you need a local scheduling hint, use get_next_task, but keep course explanations grounded in the selected course repository."
    ) -join [Environment]::NewLine
    $prompt | Set-Content -LiteralPath $promptPath -Encoding UTF8

    $checklist = @(
        "# Polaris Learned auto-connect checklist",
        "",
        "## Generated paths",
        "",
        "- learned_root: $LearnedRoot",
        "- projects_found: $($projects.Count)",
        "- default_project_id: $defaultProjectId",
        "- default_project_root: $defaultProjectRoot",
        "- command: $polaris",
        "- db: $DbPath",
        "- config: $configPath",
        "- prompt: $promptPath",
        "- projects: $projectsPath",
        "- checklist: $checklistPath",
        "",
        "## Setup steps",
        "",
        "1. Copy the ``mcpServers.polaris-core`` block from ``mcp-config.json`` into your AI IDE MCP configuration.",
        "2. Open the AI IDE at ``$LearnedRoot``.",
        "3. Restart or refresh the AI IDE MCP server list.",
        "4. Paste ``start-from-learned-prompt.md`` into the AI conversation.",
        "5. Ask the AI to call ``discover_learning_projects`` first. You should see at least ``$defaultProjectId``.",
        "6. Ask the AI to call ``detect_project_manifest`` with the selected ``project_root``.",
        "7. Ask the AI to call ``get_ai_interaction_profile`` and follow ``guidance``.",
        "8. Paste a learning note and ask the AI to save it with ``capture_evidence``. Confirm ``recorded_only=true``.",
        "9. Ask the AI to call ``list_learner_inbox``, then ``act_on_learner_inbox_item(action=accept)`` and ``draft_inbox_practice``.",
        "10. Answer the generated practice item, then ask the AI to call ``submit_inbox_practice`` with your answer and confidence.",
        "11. Ask the AI to call ``get_learner_mirror`` and confirm this attempt appears in the mirror.",
        "",
        "## Notes",
        "",
        "- You do not need one MCP per course. The same Polaris MCP scans the learning root and then attaches to the selected course.",
        "- ``discover_learning_projects`` is read-only. It does not write to ``$LearnedRoot``.",
        "- For a real long-lived database, change the value after ``--db`` to ``C:\MyProject\polaris-data\polaris.sqlite`` after initializing that database."
    ) -join [Environment]::NewLine
    $checklist | Set-Content -LiteralPath $checklistPath -Encoding UTF8

    Write-Host "P14D Learned auto-connect kit generated."
    Write-Host "learned_root: $LearnedRoot"
    Write-Host "projects_found: $($projects.Count)"
    Write-Host "default_project_id: $defaultProjectId"
    Write-Host "default_project_root: $defaultProjectRoot"
    Write-Host "command: $polaris"
    Write-Host "db: $DbPath"
    Write-Host "cwd: $LearnedRoot"
    Write-Host "config: $configPath"
    Write-Host "prompt: $promptPath"
    Write-Host "projects: $projectsPath"
    Write-Host "checklist: $checklistPath"
}
finally {
    Pop-Location
}

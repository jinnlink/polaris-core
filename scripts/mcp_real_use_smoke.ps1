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
$utf8 = [System.Text.Encoding]::UTF8
$invariantCulture = [System.Globalization.CultureInfo]::InvariantCulture
$mcpReadTimeoutMs = 15000
$mcpHeaderMaxBytes = 8192
$mcpStderrLines = New-Object 'System.Collections.Concurrent.ConcurrentQueue[string]'
$mcpStderrHandler = [System.Diagnostics.DataReceivedEventHandler] {
    param($Sender, $EventArgs)

    if ($null -ne $EventArgs.Data) {
        $script:mcpStderrLines.Enqueue($EventArgs.Data)
    }
}

if (-not $DbPath) {
    $DbPath = Join-Path $repoRoot "target\p14b-mcp-real-use.sqlite"
}
if (-not $ProjectPath) {
    $ProjectPath = Join-Path $repoRoot "examples\project-manifests\rust-mastery-lab"
}
if (-not $TranscriptPath) {
    $TranscriptPath = Join-Path $repoRoot "target\p14b-mcp-real-use-transcript.txt"
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
New-Item -ItemType Directory -Force (Split-Path -Parent $DbPath) | Out-Null
New-Item -ItemType Directory -Force (Split-Path -Parent $TranscriptPath) | Out-Null

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

function Run-LoggedCommand {
    param(
        [string]$Command,
        [string[]]$Arguments
    )

    Add-TranscriptLine ""
    Add-TranscriptLine ("> " + $Command + " " + ($Arguments -join " "))

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Command @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    foreach ($line in $output) {
        Add-TranscriptLine $line.ToString()
    }
    if ($exitCode -ne 0) {
        throw "$Command failed with exit code $exitCode"
    }
    return $output
}

function ConvertTo-CompactJson {
    param([object]$Value)

    return ($Value | ConvertTo-Json -Depth 30 -Compress)
}

function Send-McpMessage {
    param(
        [System.Diagnostics.Process]$Process,
        [hashtable]$Message
    )

    $json = ConvertTo-CompactJson $Message
    $bodyBytes = $utf8.GetBytes($json)
    $headerBytes = $utf8.GetBytes("Content-Length: $($bodyBytes.Length)`r`n`r`n")
    $stream = $Process.StandardInput.BaseStream
    $stream.Write($headerBytes, 0, $headerBytes.Length)
    $stream.Write($bodyBytes, 0, $bodyBytes.Length)
    $stream.Flush()
    $messageId = "notification"
    if ($Message.ContainsKey("id")) {
        $messageId = $Message.id
    }
    Add-TranscriptLine ""
    Add-TranscriptLine "> MCP $($Message.method) id=$messageId"
    Add-TranscriptLine $json
}

function Send-McpNotification {
    param(
        [System.Diagnostics.Process]$Process,
        [string]$Method,
        [hashtable]$Params = @{}
    )

    Send-McpMessage $Process @{ jsonrpc = "2.0"; method = $Method; params = $Params }
}

function Read-StreamBytes {
    param(
        [System.IO.Stream]$Stream,
        [byte[]]$Buffer,
        [int]$Offset,
        [int]$Count,
        [string]$Context
    )

    $readTask = $Stream.ReadAsync($Buffer, $Offset, $Count)
    if (-not $readTask.Wait($mcpReadTimeoutMs)) {
        throw "Timed out after ${mcpReadTimeoutMs}ms while $Context"
    }
    return $readTask.Result
}

function Read-Byte {
    param([System.IO.Stream]$Stream)

    $buffer = New-Object byte[] 1
    $read = Read-StreamBytes $Stream $buffer 0 1 "reading MCP response header"
    if ($read -le 0) {
        throw "MCP process closed stdout unexpectedly"
    }
    return [byte]$buffer[0]
}

function Read-McpMessage {
    param([System.Diagnostics.Process]$Process)

    $stream = $Process.StandardOutput.BaseStream
    $headerBytes = New-Object System.Collections.Generic.List[byte]
    while ($true) {
        $b = Read-Byte $stream
        $headerBytes.Add($b)
        if ($headerBytes.Count -gt $mcpHeaderMaxBytes) {
            throw "MCP response header exceeded $mcpHeaderMaxBytes bytes"
        }
        $count = $headerBytes.Count
        if ($count -ge 4 -and
            $headerBytes[$count - 4] -eq 13 -and
            $headerBytes[$count - 3] -eq 10 -and
            $headerBytes[$count - 2] -eq 13 -and
            $headerBytes[$count - 1] -eq 10) {
            break
        }
    }

    $header = $utf8.GetString($headerBytes.ToArray())
    $match = [regex]::Match($header, '(?im)^Content-Length:\s*(\d+)\s*$')
    if (-not $match.Success) {
        throw "MCP response missing Content-Length header: $header"
    }
    $length = [int]$match.Groups[1].Value
    $body = New-Object byte[] $length
    $offset = 0
    while ($offset -lt $length) {
        $read = Read-StreamBytes $stream $body $offset ($length - $offset) "reading MCP response body"
        if ($read -le 0) {
            throw "MCP process closed stdout while reading response body"
        }
        $offset += $read
    }

    $json = $utf8.GetString($body)
    Add-TranscriptLine "< MCP response"
    Add-TranscriptLine $json
    return ($json | ConvertFrom-Json)
}

function Invoke-Mcp {
    param(
        [System.Diagnostics.Process]$Process,
        [int]$Id,
        [string]$Method,
        [hashtable]$Params = @{}
    )

    Send-McpMessage $Process @{ jsonrpc = "2.0"; id = $Id; method = $Method; params = $Params }
    $response = Read-McpMessage $Process
    if ($response.id -ne $Id) {
        throw "Expected MCP response id $Id, got $($response.id)"
    }
    if (($response.PSObject.Properties.Name -contains "error") -and $null -ne $response.error) {
        throw "MCP method $Method returned error: $($response.error.message)"
    }
    return $response
}

function Invoke-McpTool {
    param(
        [System.Diagnostics.Process]$Process,
        [int]$Id,
        [string]$Name,
        [hashtable]$Arguments = @{}
    )

    $response = Invoke-Mcp $Process $Id "tools/call" @{ name = $Name; arguments = $Arguments }
    if (($response.result.PSObject.Properties.Name -contains "isError") -and $response.result.isError -eq $true) {
        throw "MCP tool $Name returned error: $($response.result.content[0].text)"
    }
    return ($response.result.content[0].text | ConvertFrom-Json)
}

function Assert-Text {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Start-McpProcess {
    param([string]$Polaris)

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $Polaris
    $startInfo.Arguments = "--db `"$DbPath`" mcp"
    $startInfo.WorkingDirectory = $ProjectPath
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.CreateNoWindow = $true
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    $process.add_ErrorDataReceived($mcpStderrHandler)
    if (-not $process.Start()) {
        throw "failed to start MCP process"
    }
    $process.BeginErrorReadLine()
    return $process
}

Push-Location $repoRoot
$process = $null
$script:smokePassed = $false
try {
    Reset-SmokeDatabase $DbPath
    Remove-Item -LiteralPath $TranscriptPath -ErrorAction SilentlyContinue
    Add-TranscriptLine "# P14B MCP real-use smoke transcript"
    Add-TranscriptLine "repo: $repoRoot"
    Add-TranscriptLine "db: $DbPath"
    Add-TranscriptLine "project: $ProjectPath"

    Run-LoggedCommand "cargo" @("build", "-p", "polaris-cli") | Out-Null

    $polaris = Join-Path $repoRoot "target\debug\polaris.exe"
    if (-not (Test-Path -LiteralPath $polaris)) {
        throw "polaris.exe was not built at $polaris"
    }

    Run-LoggedCommand $polaris @("--db", $DbPath, "init", "--pack", "packs\rust") | Out-Null

    $process = Start-McpProcess $polaris

    $initialize = Invoke-Mcp $process 1 "initialize" @{}
    Assert-Text ($initialize.result.serverInfo.name -eq "polaris-core") "initialize did not return polaris-core serverInfo"
    Add-TranscriptLine "initialize: $($initialize.result.serverInfo.name)"
    Send-McpNotification $process "notifications/initialized" @{}
    Add-TranscriptLine "initialized notification sent"

    $tools = Invoke-Mcp $process 2 "tools/list" @{}
    $toolNames = @($tools.result.tools | ForEach-Object { $_.name })
    foreach ($required in @(
        "detect_project_manifest",
        "get_ai_interaction_profile",
        "update_ai_interaction_profile",
        "capture_evidence",
        "list_learner_inbox",
        "act_on_learner_inbox_item",
        "draft_inbox_practice",
        "submit_inbox_practice",
        "get_learner_mirror",
        "get_next_task",
        "submit_task_response"
    )) {
        Assert-Text ($toolNames -contains $required) "tools/list missing $required"
    }
    Add-TranscriptLine ("tools/list: " + ($toolNames -join ", "))

    $project = Invoke-McpTool $process 3 "detect_project_manifest" @{}
    Assert-Text ($project.found -eq $true) "detect_project_manifest did not find a project"
    Assert-Text ($project.manifest.project_id -eq "rust-mastery-lab") "unexpected project_id"
    Assert-Text ($project.manifest.default_pack -eq "rust") "unexpected default_pack"
    Add-TranscriptLine "project_id: $($project.manifest.project_id)"
    Add-TranscriptLine "default_pack: $($project.manifest.default_pack)"
    Add-TranscriptLine "today_command: $($project.manifest.entry.today_command)"
    Add-TranscriptLine "root: $($project.project_root)"

    $updatedProfile = Invoke-McpTool $process 4 "update_ai_interaction_profile" @{
        persona = "socratic_tutor"
        verbosity = "detailed"
        explanation_depth = "examples_first"
        proactivity = "stuck_only"
        intervention_frequency = "normal"
        correction_style = "guided"
    }
    Assert-Text ($updatedProfile.persona -eq "socratic_tutor") "updated AI profile missing expected persona"
    Assert-Text ($updatedProfile.verbosity -eq "detailed") "updated AI profile missing expected verbosity"
    Assert-Text (-not [string]::IsNullOrWhiteSpace($updatedProfile.guidance)) "updated AI profile missing guidance"

    $profile = Invoke-McpTool $process 5 "get_ai_interaction_profile" @{}
    Assert-Text ($profile.persona -eq "socratic_tutor") "get_ai_interaction_profile did not persist persona"
    Add-TranscriptLine "ai_profile: $($profile.persona), $($profile.verbosity), $($profile.intervention_frequency)"

    $issuedTask = Invoke-McpTool $process 6 "get_next_task" @{
        session = "p15b-mcp-turn"
    }
    Assert-Text (-not [string]::IsNullOrWhiteSpace($issuedTask.task_event_id)) "get_next_task did not return task_event_id"
    Assert-Text (-not [string]::IsNullOrWhiteSpace($issuedTask.task.concept_id)) "get_next_task did not return a concept"
    $taskEventId = $issuedTask.task_event_id
    Add-TranscriptLine "task_event_id: $taskEventId"
    Add-TranscriptLine "task_concept: $($issuedTask.task.concept_id)"

    $turnReceipt = Invoke-McpTool $process 7 "submit_task_response" @{
        session = "p15b-mcp-turn"
        task_event_id = $taskEventId
        response = "A learner answer submitted through the issued Polaris task receipt."
        confidence = 4
    }
    Assert-Text ($turnReceipt.task_event_id -eq $taskEventId) "submit_task_response did not echo task_event_id"
    Assert-Text (-not [string]::IsNullOrWhiteSpace($turnReceipt.attempt_id)) "submit_task_response did not return attempt_id"
    Add-TranscriptLine "turn_attempt_id: $($turnReceipt.attempt_id)"

    $capture = Invoke-McpTool $process 8 "capture_evidence" @{
        session = "p14b-mcp"
        source = "mcp-real-use-smoke"
        content_type = "text/plain"
        learner_kind = "reference"
        text = "I learned through MCP that Rust ownership decides which binding drops a value."
        candidate_concept_ids = @("ownership")
    }
    Assert-Text ($capture.recorded_only -eq $true) "capture_evidence was not recorded_only"
    Assert-Text ($capture.status -eq "pending") "capture_evidence did not return pending"
    $captureId = $capture.capture_id
    Add-TranscriptLine "capture_id: $captureId"
    Add-TranscriptLine "recorded_only: $($capture.recorded_only.ToString().ToLowerInvariant())"

    $inbox = Invoke-McpTool $process 9 "list_learner_inbox" @{}
    Assert-Text (@($inbox.items | Where-Object { $_.capture_id -eq $captureId }).Count -eq 1) "list_learner_inbox did not include capture"

    $acted = Invoke-McpTool $process 10 "act_on_learner_inbox_item" @{
        capture_id = $captureId
        action = "accept"
    }
    Assert-Text ($acted.status -eq "practice_ready") "act_on_learner_inbox_item did not mark practice_ready"
    Add-TranscriptLine "status: $($acted.status)"

    $draft = Invoke-McpTool $process 11 "draft_inbox_practice" @{
        capture_id = $captureId
    }
    Assert-Text ($draft.prompt -match "Ownership") "draft_inbox_practice prompt did not mention Ownership"
    Add-TranscriptLine "prompt: $($draft.prompt)"

    $submitted = Invoke-McpTool $process 12 "submit_inbox_practice" @{
        capture_id = $captureId
        session = "p14b-mcp"
        response = "Ownership means one binding owns a value and is responsible for dropping it at scope end."
        confidence = 4
    }
    Assert-Text ($submitted.effect -eq "submitted") "submit_inbox_practice did not submit"
    $scoreDelta = [Math]::Abs([double]$submitted.provisional_score - 0.7)
    Assert-Text ($scoreDelta -lt 0.000001) "submit_inbox_practice provisional score was not 0.7"
    $attemptId = $submitted.attempt_id
    Add-TranscriptLine "attempt_id: $attemptId"
    Add-TranscriptLine ("provisional_score: " + ([double]$submitted.provisional_score).ToString("0.000", $invariantCulture))

    $mirror = Invoke-McpTool $process 13 "get_learner_mirror" @{}
    Assert-Text (@($mirror.confidence_curve).Count -ge 1) "get_learner_mirror missing confidence_curve"
    Assert-Text (@($mirror.confidence_curve | Where-Object { $_.concept_id -eq "ownership" }).Count -ge 1) "learner mirror missing ownership attempt"
    Add-TranscriptLine "confidence_curve: $(@($mirror.confidence_curve).Count)"

    Write-Host "capture_id: $captureId"
    Write-Host "attempt_id: $attemptId"
    Write-Host "P14B MCP real-use smoke passed."
    Write-Host "transcript: $TranscriptPath"
    $script:smokePassed = $true
}
finally {
    if ($null -ne $process) {
        try {
            try {
                $process.StandardInput.BaseStream.Close()
            }
            catch {
            }
            if (-not $process.WaitForExit(5000)) {
                Add-TranscriptLine "MCP process did not exit after stdin close; killing child process."
                $process.Kill()
                $process.WaitForExit()
            }
            try {
                $process.CancelErrorRead()
            }
            catch {
            }
            $stderrLines = @($mcpStderrLines.ToArray())
            if ($stderrLines.Count -gt 0) {
                Add-TranscriptLine ""
                Add-TranscriptLine "# MCP stderr"
                foreach ($line in $stderrLines) {
                    Add-TranscriptLine $line
                }
            }
            try {
                $process.remove_ErrorDataReceived($mcpStderrHandler)
            }
            catch {
            }
            $process.Dispose()
        }
        catch {
            try {
                Add-TranscriptLine "MCP process cleanup warning: $($_.Exception.Message)"
            }
            catch {
            }
        }
    }
    Pop-Location
}

if ($script:smokePassed) {
    [System.Environment]::Exit(0)
}

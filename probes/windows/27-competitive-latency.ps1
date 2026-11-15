#Requires -Version 5.1
<#
.SYNOPSIS
    Area 27 competitive-latency.ps1 - model-free A/B of whole agent-desktop CLI
    invocations between two or more builds on the same Windows host.

.DESCRIPTION
    Each arm is one agent-desktop release binary:

      fork      the checkout this script runs from
      upstream  a shallow clone of the upstream repository's default branch
      forkmain  a shallow clone of this fork's main branch, the before side
                of a before/after for a change on the current checkout

    With -Build the script clones and builds every arm itself, the shipped
    release profile, one target directory per arm, all builds at once so the
    serial LTO tails overlap. Without -Build it expects each arm's binary at
    <WorkRoot>\<arm>\agent-desktop.exe.

    Legs, per target window, with the arms interleaved sample by sample and
    the arm order alternating every sample so neither arm always runs first
    on a warm cache:

      snapshot  snapshot --window-id <window> -i
      find      find --first --window-id <window> --native-id <button>
      click     click <ref>          (ref from the arm's own untimed find)
      type      --headed type <ref> q  (ref from the arm's own untimed find;
                                        Windows type is keystroke synthesis and
                                        is refused headless; before every sample
                                        an untimed --headed focus on the button
                                        takes keyboard focus off the field)
      press     --headed press q --ref <ref>  (the same text box, the same
                                        untimed focus reset before every sample)

    and once per host: list-apps, and launch <fixture exe> with every
    instance the launch started killed, untimed, before the next sample.

    Targets are the repository's own fixtures: the WinForms fixture built
    from tests/fixture-app-windows (Win32 controls through the UIA proxy) and
    the WPF scratch window from scratch/ScratchWpf.ps1 (a native UIA
    provider). Each arm keeps its own AGENT_DESKTOP_HOME so one arm's refmaps
    and sessions never serve the other.

    Method (A15-13, A18-7): one discarded warm-up sample then seven timed
    samples per arm and leg, reported as min with median and max beside it.
    Every sample records the envelope's ok, so a fast failure cannot pass as
    a fast success, and the click and type legs are checked afterwards by an
    independent UI Automation read of the fixture's own state.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\27-competitive-latency.ps1 -Label <devbox|ci> -Build
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'mergebase', 'ci')][string]$Label = 'devbox',
    [string[]]$Arm = @('fork', 'upstream'),
    [string]$UpstreamRepository = 'https://github.com/vercel-labs/agent-desktop.git',
    [string]$ForkRepository = 'https://github.com/gshost1/agent-desktop.git',
    [string]$WorkRoot = '',
    [switch]$Build,
    [int]$TimedRuns = 7
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) 'common.ps1')
Initialize-ProbeRedaction

$Probe = '27-competitive-latency'
$CaptureName = "competitive-latency-$Label.json"
Register-MandatoryCapture -Name @($CaptureName)

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:RepoRoot = Split-Path -Parent (Split-Path -Parent $script:ProbeDir)
if (-not $WorkRoot) {
    $base = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
    $WorkRoot = Join-Path $base 'agent-desktop-competitive'
}
New-Item -ItemType Directory -Path $WorkRoot -Force | Out-Null

function Get-ArmSource {
    param([Parameter(Mandatory = $true)][string]$Name)
    switch ($Name) {
        'fork' { return [pscustomobject]@{ Name = $Name; Repository = ''; Checkout = $script:RepoRoot } }
        'upstream' { return [pscustomobject]@{ Name = $Name; Repository = $UpstreamRepository; Checkout = (Join-Path $WorkRoot 'src-upstream') } }
        'forkmain' { return [pscustomobject]@{ Name = $Name; Repository = $ForkRepository; Checkout = (Join-Path $WorkRoot 'src-forkmain') } }
        default { throw "unknown arm $Name" }
    }
}

function Get-ArmBinary {
    param([Parameter(Mandatory = $true)][string]$Name)
    return (Join-Path (Join-Path $WorkRoot $Name) 'agent-desktop.exe')
}

function Get-CheckoutCommit {
    param([Parameter(Mandatory = $true)][string]$Checkout)
    $commit = & git -C $Checkout rev-parse HEAD 2>$null
    if ($LASTEXITCODE -ne 0) { return '' }
    return ([string]$commit).Trim()
}

function Invoke-ArmBuilds {
    param([Parameter(Mandatory = $true)][object[]]$Sources)
    $running = New-Object System.Collections.ArrayList
    foreach ($source in $Sources) {
        if ($source.Repository) {
            if (Test-Path -LiteralPath $source.Checkout) { Remove-Item -LiteralPath $source.Checkout -Recurse -Force }
            & git clone --quiet --depth 1 $source.Repository $source.Checkout
            if ($LASTEXITCODE -ne 0) { throw ('clone failed for arm ' + $source.Name) }
        }
        $targetDir = Join-Path $WorkRoot ('target-' + $source.Name)
        $log = Join-Path $WorkRoot ('build-' + $source.Name + '.log')
        $command = 'cd /d "' + $source.Checkout + '" && rustup show >nul && cargo build --locked --release -p agent-desktop --target-dir "' + $targetDir + '" > "' + $log + '" 2>&1'
        $process = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/c', $command) -PassThru -WindowStyle Hidden
        [void]$running.Add([pscustomobject]@{ Source = $source; Process = $process; TargetDir = $targetDir; Log = $log; Started = [DateTime]::UtcNow })
    }
    $timings = [ordered]@{}
    foreach ($entry in $running) {
        $entry.Process.WaitForExit()
        $seconds = [Math]::Round(([DateTime]::UtcNow - $entry.Started).TotalSeconds, 1)
        if ($entry.Process.ExitCode -ne 0) {
            Get-Content -LiteralPath $entry.Log -Tail 40 | ForEach-Object { Write-Host $_ }
            throw ('release build failed for arm ' + $entry.Source.Name)
        }
        $armDir = Join-Path $WorkRoot $entry.Source.Name
        New-Item -ItemType Directory -Path $armDir -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $entry.TargetDir 'release\agent-desktop.exe') -Destination (Get-ArmBinary -Name $entry.Source.Name) -Force
        $timings[$entry.Source.Name] = $seconds
        Write-Host ('built arm ' + $entry.Source.Name + ' in ' + $seconds + ' s')
    }
    return $timings
}

function ConvertTo-ArgumentLine {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    return (($Arguments | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' ')
}

function Invoke-ArmCommand {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [int]$TimeoutMs = 60000
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = Get-ArmBinary -Name $Name
    $psi.Arguments = ConvertTo-ArgumentLine -Arguments $Arguments
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $psi.EnvironmentVariables['AGENT_DESKTOP_HOME'] = Join-Path $WorkRoot ('home-' + $Name)
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $process = [System.Diagnostics.Process]::Start($psi)
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutMs)) {
        try { $process.Kill() } catch { }
        throw ('arm ' + $Name + ' timed out after ' + $TimeoutMs + ' ms: ' + $Arguments[0])
    }
    $process.WaitForExit()
    $watch.Stop()
    $stdout = $stdoutTask.Result
    $null = $stderrTask.Result
    $envelope = $null
    try { $envelope = $stdout | ConvertFrom-Json } catch { $envelope = $null }
    $ok = ($null -ne $envelope) -and ($envelope.PSObject.Properties.Name -contains 'ok') -and [bool]$envelope.ok
    $code = ''
    $failure = $null
    if ($null -ne $envelope -and -not $ok -and ($envelope.PSObject.Properties.Name -contains 'error') -and $envelope.error) {
        $code = [string]$envelope.error.code
        $failure = [ordered]@{ code = $code; message = [string]$envelope.error.message }
        if (($envelope.error.PSObject.Properties.Name -contains 'details') -and $envelope.error.details -and ($envelope.error.details.PSObject.Properties.Name -contains 'kind')) {
            $failure['kind'] = [string]$envelope.error.details.kind
        }
    }
    return [pscustomobject]@{ Ms = [double]$watch.Elapsed.TotalMilliseconds; Exit = $process.ExitCode; Ok = $ok; Code = $code; Failure = $failure; Envelope = $envelope }
}

function Get-SampleSummary {
    param([Parameter(Mandatory = $true)][object[]]$Samples)
    $timed = @($Samples | Select-Object -Skip 1)
    $sorted = @($timed | ForEach-Object { $_.Ms } | Sort-Object)
    $codes = @($timed | Where-Object { -not $_.Ok -and $_.Code } | ForEach-Object { $_.Code } | Sort-Object -Unique)
    $firstFailure = @($Samples | Where-Object { $null -ne $_.Failure } | Select-Object -First 1 | ForEach-Object { $_.Failure })
    $firstOkSteps = @()
    $firstOk = @($Samples | Where-Object { $_.Ok } | Select-Object -First 1)
    if ($firstOk.Count -gt 0 -and ($firstOk[0].Envelope.data.PSObject.Properties.Name -contains 'steps')) {
        $firstOkSteps = @($firstOk[0].Envelope.data.steps | ForEach-Object { [string]$_.label })
    }
    return [ordered]@{
        timed_runs       = $sorted.Count
        warmup_discarded = 1
        warmup_ms        = [Math]::Round([double]$Samples[0].Ms, 1)
        min_ms           = [Math]::Round([double]$sorted[0], 1)
        median_ms        = [Math]::Round([double]$sorted[[int][Math]::Floor($sorted.Count / 2)], 1)
        max_ms           = [Math]::Round([double]$sorted[$sorted.Count - 1], 1)
        ok_runs          = @($timed | Where-Object { $_.Ok }).Count
        ok_with_warmup   = @($Samples | Where-Object { $_.Ok }).Count
        error_codes      = $codes
        first_failure    = $(if ($firstFailure.Count -gt 0) { $firstFailure[0] } else { $null })
        first_ok_steps   = $firstOkSteps
    }
}

<#
    Runs one leg on every arm, interleaved: sample i runs every arm once, in
    an order that rotates with i. ArgumentsByArm maps an arm to its argument
    list, because ref-bearing legs carry each arm's own ref. ReapProcessName,
    when given, kills every process of that name that the leg started, untimed,
    after every sample. ResetArguments, when given, runs untimed through the
    first arm before every sample, so a state one arm's sample leaves behind
    (keyboard focus, for type) cannot serve the next arm.
#>
function Measure-Leg {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arms,
        [Parameter(Mandatory = $true)][hashtable]$ArgumentsByArm,
        [string]$ReapProcessName = '',
        [string[]]$ResetArguments = @()
    )
    $baseline = @()
    if ($ReapProcessName) { $baseline = @(Get-Process -Name $ReapProcessName -ErrorAction SilentlyContinue | ForEach-Object { $_.Id }) }
    $samples = @{}
    foreach ($name in $Arms) { $samples[$name] = New-Object System.Collections.ArrayList }
    for ($i = 0; $i -le $TimedRuns; $i++) {
        for ($j = 0; $j -lt $Arms.Count; $j++) {
            $name = $Arms[($i + $j) % $Arms.Count]
            if ($ResetArguments.Count -gt 0) { $null = Invoke-ArmCommand -Name $Arms[0] -Arguments $ResetArguments }
            [void]$samples[$name].Add((Invoke-ArmCommand -Name $name -Arguments $ArgumentsByArm[$name]))
            if ($ReapProcessName) {
                Get-Process -Name $ReapProcessName -ErrorAction SilentlyContinue |
                    Where-Object { $baseline -notcontains $_.Id } |
                    ForEach-Object { Stop-ScratchProcess -ProcessId $_.Id }
            }
        }
    }
    $result = [ordered]@{}
    foreach ($name in $Arms) { $result[$name] = Get-SampleSummary -Samples @($samples[$name]) }
    return $result
}

function Get-ArmReference {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$WindowArgument,
        [Parameter(Mandatory = $true)][string]$NativeId
    )
    $found = Invoke-ArmCommand -Name $Name -Arguments @('find', '--first', '--window-id', $WindowArgument, '--native-id', $NativeId)
    if (-not $found.Ok) { return '' }
    $data = $found.Envelope.data
    if (($data.PSObject.Properties.Name -contains 'match') -and $data.match) { return [string]$data.match.ref_id }
    if (($data.PSObject.Properties.Name -contains 'matches') -and @($data.matches).Count -gt 0) { return [string]@($data.matches)[0].ref_id }
    return ''
}

function Initialize-UiaClient {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
}

function Get-UiaElement {
    param([Parameter(Mandatory = $true)][IntPtr]$WindowHandle, [Parameter(Mandatory = $true)][string]$Key)
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($WindowHandle)
    $condition = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::AutomationIdProperty), $Key
    return $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}

<#
    The managed UI Automation client reads WinForms controls through its own
    client-side proxies, which surface neither Control.Name as AutomationId
    nor AccessibleName as Name the way the native UIA3 proxy does. A WinForms
    readout is therefore read the way tests/e2e-windows reads it: a separate
    find of the readout by native id, never the command being measured.
#>
function Get-ReadoutText {
    param([Parameter(Mandatory = $true)][IntPtr]$WindowHandle, [Parameter(Mandatory = $true)][string]$Key, [switch]$ThroughFind)
    if (-not $ThroughFind) { return (Get-UiaText -WindowHandle $WindowHandle -Key $Key) }
    $found = Invoke-ArmCommand -Name $Arm[0] -Arguments @('find', '--first', '--window-id', ('w-' + [long]$WindowHandle), '--native-id', $Key)
    if (-not $found.Ok -or -not ($found.Envelope.data.PSObject.Properties.Name -contains 'match') -or -not $found.Envelope.data.match) { return $null }
    $match = $found.Envelope.data.match
    if ($match.PSObject.Properties.Name -contains 'value') { return [string]$match.value }
    return $null
}

function Get-UiaText {
    param([Parameter(Mandatory = $true)][IntPtr]$WindowHandle, [Parameter(Mandatory = $true)][string]$Key)
    $element = Get-UiaElement -WindowHandle $WindowHandle -Key $Key
    if ($null -eq $element) { return $null }
    $pattern = $null
    if ($element.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
        return [string]$pattern.Current.Value
    }
    return [string]$element.Current.Name
}

function Get-CharacterCount {
    param([AllowNull()][string]$Text, [Parameter(Mandatory = $true)][char]$Character)
    if ($null -eq $Text) { return -1 }
    return @($Text.ToCharArray() | Where-Object { $_ -eq $Character }).Count
}

function Get-TotalOkRuns {
    param([Parameter(Mandatory = $true)]$Leg)
    $total = 0
    foreach ($key in $Leg.Keys) { $total += [int]$Leg[$key].ok_with_warmup }
    return $total
}

<#
    Reads one of the fixture's own counters after a leg. Evidence names the
    control (Key, and ThroughFind for a WinForms readout) and the text it shows after n deliveries (Format,
    with {0} for n). Both fixtures count every delivery: the WinForms status
    readouts append a per-write counter, and the WPF window counts clicks.
#>
function Get-CounterEvidence {
    param([Parameter(Mandatory = $true)]$Target, [Parameter(Mandatory = $true)]$Evidence, [Parameter(Mandatory = $true)][int]$Expected)
    $observed = Get-ReadoutText -WindowHandle $Target.WindowHandle -Key $Evidence.Key -ThroughFind:$Evidence.ThroughFind
    $wanted = $Evidence.Format -f $Expected
    return [ordered]@{ verified = ($observed -eq $wanted); expected = $wanted; observed = $observed }
}

<#
    Measures the four ref legs against one target window. Target carries
    Name, WindowHandle, the Button and TextBox native ids, and the counters
    that independently confirm the click and type legs (ClickEvidence and
    TypeEvidence; a TypeEvidence of $null counts the typed character in the
    text box's own value instead).
#>
function Measure-TargetLegs {
    param([Parameter(Mandatory = $true)][string[]]$Arms, [Parameter(Mandatory = $true)]$Target)
    $windowArgument = 'w-' + [long]$Target.WindowHandle
    $snapshotArgs = @{}; $findArgs = @{}; $clickArgs = @{}; $typeArgs = @{}; $pressArgs = @{}
    $references = [ordered]@{}
    foreach ($name in $Arms) {
        $snapshotArgs[$name] = @('snapshot', '--window-id', $windowArgument, '-i')
        $findArgs[$name] = @('find', '--first', '--window-id', $windowArgument, '--native-id', $Target.Button)
        $button = Get-ArmReference -Name $name -WindowArgument $windowArgument -NativeId $Target.Button
        $text = Get-ArmReference -Name $name -WindowArgument $windowArgument -NativeId $Target.TextBox
        $references[$name] = [ordered]@{ button_resolved = [bool]$button; text_resolved = [bool]$text }
        $clickArgs[$name] = @('click', $(if ($button) { $button } else { '@unresolved:e1' }))
        $typeArgs[$name] = @('--headed', 'type', $(if ($text) { $text } else { '@unresolved:e1' }), 'q')
        $pressArgs[$name] = @('--headed', 'press', 'q', '--ref', $(if ($text) { $text } else { '@unresolved:e1' }))
    }
    $legs = [ordered]@{}
    $legs['snapshot'] = Measure-Leg -Arms $Arms -ArgumentsByArm $snapshotArgs
    $legs['find'] = Measure-Leg -Arms $Arms -ArgumentsByArm $findArgs
    $legs['click'] = Measure-Leg -Arms $Arms -ArgumentsByArm $clickArgs
    $clickEvidence = Get-CounterEvidence -Target $Target -Evidence $Target.ClickEvidence -Expected (Get-TotalOkRuns -Leg $legs['click'])
    $before = Get-CharacterCount -Text (Get-UiaText -WindowHandle $Target.WindowHandle -Key $Target.TextBox) -Character 'q'
    $focusElsewhere = @('--headed', 'focus', $clickArgs[$Arms[0]][1])
    $legs['type'] = Measure-Leg -Arms $Arms -ArgumentsByArm $typeArgs -ResetArguments $focusElsewhere
    $expectedTyped = Get-TotalOkRuns -Leg $legs['type']
    if ($Target.TypeEvidence) {
        $typeEvidence = Get-CounterEvidence -Target $Target -Evidence $Target.TypeEvidence -Expected $expectedTyped
    } else {
        $after = Get-CharacterCount -Text (Get-UiaText -WindowHandle $Target.WindowHandle -Key $Target.TextBox) -Character 'q'
        $typeEvidence = [ordered]@{ verified = (($after - $before) -eq $expectedTyped); expected = $expectedTyped; observed = ($after - $before) }
    }
    $beforePress = Get-CharacterCount -Text (Get-UiaText -WindowHandle $Target.WindowHandle -Key $Target.TextBox) -Character 'q'
    $legs['press'] = Measure-Leg -Arms $Arms -ArgumentsByArm $pressArgs -ResetArguments $focusElsewhere
    $expectedPressed = Get-TotalOkRuns -Leg $legs['press']
    if ($Target.TypeEvidence) {
        $pressEvidence = Get-CounterEvidence -Target $Target -Evidence $Target.TypeEvidence -Expected ($expectedTyped + $expectedPressed)
    } else {
        $afterPress = Get-CharacterCount -Text (Get-UiaText -WindowHandle $Target.WindowHandle -Key $Target.TextBox) -Character 'q'
        $pressEvidence = [ordered]@{ verified = (($afterPress - $beforePress) -eq $expectedPressed); expected = $expectedPressed; observed = ($afterPress - $beforePress) }
    }
    return [ordered]@{
        target     = $Target.Name
        references = $references
        legs       = $legs
        evidence   = [ordered]@{ click = $clickEvidence; type = $typeEvidence; press = $pressEvidence }
    }
}

function Start-Win32Target {
    param([Parameter(Mandatory = $true)][string]$FixtureExe)
    $env:AGENT_DESKTOP_FIXTURE_NO_ACTIVATE = '1'
    $instance = Start-ScratchProcess -FilePath $FixtureExe -NoActivate -TimeoutSec 30
    if ($instance.MainWindowHandle -eq [IntPtr]::Zero) { throw 'the WinForms fixture produced no main window' }
    $handle = $instance.MainWindowHandle
    return [pscustomobject]@{
        Name          = 'win32-winforms-fixture'
        ProcessId     = $instance.ProcessId
        WindowHandle  = $handle
        Button        = 'primary-button'
        TextBox       = 'text-input'
        ClickEvidence = [pscustomobject]@{ Key = 'click-status'; ThroughFind = $true; Format = 'clicked#{0}' }
        TypeEvidence  = [pscustomobject]@{ Key = 'text-status'; ThroughFind = $true; Format = 'changed#{0}' }
    }
}

function Start-WpfTarget {
    $script = Join-Path $script:ProbeDir 'scratch\ScratchWpf.ps1'
    $process = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $script, '-Tag', 'competitive', '-TimeoutSeconds', '1500') -PassThru -WindowStyle Hidden
    Register-ScratchProcessId -ProcessId $process.Id
    $handle = [IntPtr]::Zero
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        Start-Sleep -Milliseconds 500
        $process.Refresh()
        if ($process.HasExited) { break }
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) { $handle = $process.MainWindowHandle; break }
    }
    if ($handle -eq [IntPtr]::Zero) { throw 'the WPF scratch window produced no main window' }
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline -and $null -eq (Get-UiaElement -WindowHandle $handle -Key 'btnAction')) { Start-Sleep -Milliseconds 250 }
    return [pscustomobject]@{
        Name          = 'wpf-scratch-window'
        ProcessId     = $process.Id
        WindowHandle  = $handle
        Button        = 'btnAction'
        TextBox       = 'txtValue'
        ClickEvidence = [pscustomobject]@{ Key = 'txtStatusMirror'; ThroughFind = $false; Format = 'action:{0}' }
        TypeEvidence  = $null
    }
}

function Measure-LaunchLeg {
    param([Parameter(Mandatory = $true)][string[]]$Arms, [Parameter(Mandatory = $true)][string]$FixtureExe)
    $processName = [System.IO.Path]::GetFileNameWithoutExtension($FixtureExe)
    $launchArgs = @{}
    foreach ($name in $Arms) { $launchArgs[$name] = @('launch', $FixtureExe, '--timeout', '20000') }
    return (Measure-Leg -Arms $Arms -ArgumentsByArm $launchArgs -ReapProcessName $processName)
}

function Get-RatioTable {
    param([Parameter(Mandatory = $true)]$Capture, [Parameter(Mandatory = $true)][string[]]$Arms)
    $rows = New-Object System.Collections.ArrayList
    $sections = New-Object System.Collections.ArrayList
    foreach ($target in $Capture.targets) { [void]$sections.Add(@($target.target, $target.legs)) }
    [void]$sections.Add(@('host', $Capture.host_legs))
    foreach ($section in $sections) {
        foreach ($legName in $section[1].Keys) {
            $leg = $section[1][$legName]
            $row = [ordered]@{ target = $section[0]; leg = $legName }
            foreach ($name in $Arms) {
                $row[$name] = ('{0} / {1} / {2} ({3}/{4} ok)' -f $leg[$name].min_ms, $leg[$name].median_ms, $leg[$name].max_ms, $leg[$name].ok_runs, $leg[$name].timed_runs)
            }
            if ($Arms.Count -ge 2 -and $leg[$Arms[1]].median_ms -gt 0) {
                $row['median_ratio'] = [Math]::Round($leg[$Arms[0]].median_ms / $leg[$Arms[1]].median_ms, 2)
                $row['min_ratio'] = [Math]::Round($leg[$Arms[0]].min_ms / $leg[$Arms[1]].min_ms, 2)
            }
            [void]$rows.Add($row)
        }
    }
    return $rows
}

$targets = New-Object System.Collections.ArrayList
$started = New-Object System.Collections.ArrayList
try {
    $sources = @($Arm | ForEach-Object { Get-ArmSource -Name $_ })
    $buildSeconds = $null
    if ($Build) { $buildSeconds = Invoke-ArmBuilds -Sources $sources }
    foreach ($name in $Arm) {
        if (-not (Test-Path -LiteralPath (Get-ArmBinary -Name $name))) { throw ('no binary for arm ' + $name) }
    }
    $armInfo = [ordered]@{}
    foreach ($source in $sources) {
        $version = Invoke-ArmCommand -Name $source.Name -Arguments @('version')
        $armInfo[$source.Name] = [ordered]@{
            commit       = Get-CheckoutCommit -Checkout $source.Checkout
            version      = $(if ($version.Ok) { [string]$version.Envelope.data.version } else { '' })
            binary_bytes = (Get-Item -LiteralPath (Get-ArmBinary -Name $source.Name)).Length
        }
    }

    Initialize-UiaClient
    $fixtureOut = Join-Path $WorkRoot 'fixture'
    & (Join-Path $script:RepoRoot 'tests\fixture-app-windows\build.ps1') -OutputDir $fixtureOut | Out-Null
    $fixtureExe = Join-Path $fixtureOut 'AgentDeskFixture.exe'
    if (-not (Test-Path -LiteralPath $fixtureExe)) { throw 'the WinForms fixture did not build' }

    $win32 = Start-Win32Target -FixtureExe $fixtureExe
    [void]$started.Add($win32.ProcessId)
    [void]$targets.Add((Measure-TargetLegs -Arms $Arm -Target $win32))
    $wpf = Start-WpfTarget
    [void]$started.Add($wpf.ProcessId)
    [void]$targets.Add((Measure-TargetLegs -Arms $Arm -Target $wpf))
    foreach ($id in $started) { try { Stop-ScratchProcess -ProcessId $id } catch { } }
    $started.Clear()

    $hostLegs = [ordered]@{}
    $listArgs = @{}
    foreach ($name in $Arm) { $listArgs[$name] = @('list-apps') }
    $hostLegs['list-apps'] = Measure-Leg -Arms $Arm -ArgumentsByArm $listArgs
    $hostLegs['launch'] = Measure-LaunchLeg -Arms $Arm -FixtureExe $fixtureExe

    $capture = [ordered]@{
        probe         = $Probe
        label         = $Label
        methodology   = 'one discarded warm-up then seven timed samples per arm and leg, arms interleaved with rotating order; min with median and max beside it (A15-13, A18-7)'
        arms          = $armInfo
        build_seconds = $buildSeconds
        targets       = @($targets)
        host_legs     = $hostLegs
    }
    $capture['table'] = @(Get-RatioTable -Capture $capture -Arms $Arm)
    $capturePath = Write-ProbeJson -Probe $Probe -Name $CaptureName -InputObject $capture
    Register-MandatoryPass -Capture $capturePath -Result $capture
    Write-Host ('wrote ' + $capturePath)
    Write-Host ('arms (min / median / max ms): ' + ($Arm -join ' vs '))
    foreach ($row in $capture['table']) { Write-Host (($row.Values | ForEach-Object { [string]$_ }) -join ' | ') }
    Assert-MandatoryMeasurement -Probe $Probe -Label $Label
    Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'competitive latency captured' -Data @{ capture = "captures/$Probe/$CaptureName" }
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + ($_.Exception.Message -replace '[\r\n]+', ' '))
    exit 1
} finally {
    foreach ($id in $started) { try { Stop-ScratchProcess -ProcessId $id } catch { } }
}

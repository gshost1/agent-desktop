#Requires -Version 5.1

<#
    ForegroundContender.psm1 - a top-level window the harness owns, for legs
    whose precondition is "foreground is not the fixture". Stealing onto
    whatever other window the desktop happens to have is not reliable on a
    hosted runner: run 43805176493 found 11 visible top-level windows and
    none took foreground. The contender is a plain WinForms form in its own
    powershell.exe with its own message loop, so it answers activation like
    any application window, launched detached under the held desktop lease
    and torn down through its job object.
#>

Set-StrictMode -Version 2.0

$script:ContenderTitle = 'agent-desktop-e2e-foreground-contender'

function Start-ForegroundContender {
    <#
    .OUTPUTS
        PSCustomObject: Process (Invoke-GuardedDetached's handle) and
        WindowHandle (the contender form's visible, uncloaked HWND).
    #>
    [CmdletBinding()]
    param([int]$TimeoutSeconds = 30)
    $formScript = @"
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
`$form = New-Object System.Windows.Forms.Form
`$form.Text = '$script:ContenderTitle'
`$form.StartPosition = 'Manual'
`$form.Location = New-Object System.Drawing.Point(0, 0)
`$form.Size = New-Object System.Drawing.Size(260, 120)
`$form.Add_Shown({ `$form.Activate() })
[System.Windows.Forms.Application]::Run(`$form)
"@
    $encoded = [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($formScript))
    $powershell = Join-Path $PSHOME 'powershell.exe'
    $process = Invoke-GuardedDetached -FilePath $powershell -ArgumentList @('-NoProfile', '-NonInteractive', '-EncodedCommand', $encoded)
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    while ($deadline.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
        $window = Get-NativeTopLevelWindows | Where-Object {
            $_.ProcessId -eq $process.ProcessId -and [AgentDesktopE2E.Native]::IsWindowVisible($_.Handle) -and -not (Test-NativeWindowCloaked -WindowHandle $_.Handle)
        } | Select-Object -First 1
        if ($window) { return [pscustomobject]@{ Process = $process; WindowHandle = $window.Handle } }
        Start-Sleep -Milliseconds 100
    }
    Stop-GuardedDetached -Handle $process
    throw "Start-ForegroundContender: contender process $($process.ProcessId) showed no visible top-level window within $TimeoutSeconds s"
}

function Invoke-WithForegroundStolen {
    <#
    .SYNOPSIS
        Runs Body with OS foreground verified to be off ExcludeHandle: on the
        harness's own contender first, then on any other visible, uncloaked
        window. Throws, naming what was tried, when foreground stays put.
        The contender is stopped when Body returns or throws.
    #>
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][IntPtr]$ExcludeHandle, [Parameter(Mandatory = $true)][scriptblock]$Body)
    $contender = Start-ForegroundContender
    try {
        $taken = Set-NativeForegroundWindow -WindowHandle $contender.WindowHandle
        if (-not $taken) { $taken = [bool](Set-NativeForegroundAwayFrom -ExcludeHandle $ExcludeHandle) }
        $foreground = Get-NativeForegroundWindowHandle
        if (-not $taken -or $foreground -eq $ExcludeHandle) {
            throw "Invoke-WithForegroundStolen: foreground stayed on $ExcludeHandle (now $foreground); the harness contender $($contender.WindowHandle) and every other visible, uncloaked window refused it"
        }
        & $Body
    } finally {
        Stop-GuardedDetached -Handle $contender.Process
    }
}

Export-ModuleMember -Function @('Start-ForegroundContender', 'Invoke-WithForegroundStolen')

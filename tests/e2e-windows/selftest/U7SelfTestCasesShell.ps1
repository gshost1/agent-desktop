#Requires -Version 5.1

<#
    U7SelfTestCasesShell.ps1 - dot-sourced by Invoke-U7SelfTests.ps1. Cases
    15-18: the skip gates of the shell-surface legs (Get-ShellSurfaceAbsence,
    Resolve-ShellTrayAbsence), the foreground-steal wrapper
    (Invoke-WithForegroundStolen) and the verdict an aborted Run-E2E.ps1
    prints. A skip gate is the code best placed to hide a real failure, so
    each case inverts its own guard inline and watches the case fail.
    Set-LibFunctionText and the other helpers are the caller's own scope.
#>

function New-ShellOpen {
    param([string]$ErrorCode)
    return [pscustomobject]@{ Opened = $false; ErrorCode = $ErrorCode; WindowId = $null }
}

function New-ShellHostEvidence {
    param([bool]$HostRunning, [int]$ProductType)
    return [pscustomobject]@{ ShellHostRunning = $HostRunning; ProductType = $ProductType; Text = "product_type=$ProductType host=$HostRunning" }
}

<#
    15. Get-ShellSurfaceAbsence skips only the build refusal, or a TIMEOUT
        on a server SKU with no ShellExperienceHost. INVERT: dropping the
        workstation check makes a workstation TIMEOUT a skip.
#>
Invoke-SelfTest 'Get-ShellSurfaceAbsence skips only INVALID_ARGS or a hostless server TIMEOUT' {
    $serverHostless = New-ShellHostEvidence -HostRunning $false -ProductType 3
    Assert-True ($null -ne (Get-ShellSurfaceAbsence -Open (New-ShellOpen 'INVALID_ARGS') -Evidence $serverHostless)) 'the build refusal is an absence'
    Assert-True ((Get-ShellSurfaceAbsence -Open (New-ShellOpen 'TIMEOUT') -Evidence $serverHostless) -like '*server SKU*') 'a TIMEOUT on a hostless server is an absence, and says so'
    Assert-True ($null -eq (Get-ShellSurfaceAbsence -Open (New-ShellOpen 'TIMEOUT') -Evidence (New-ShellHostEvidence -HostRunning $false -ProductType 1))) 'a TIMEOUT on a workstation is a failure'
    Assert-True ($null -eq (Get-ShellSurfaceAbsence -Open (New-ShellOpen 'TIMEOUT') -Evidence (New-ShellHostEvidence -HostRunning $true -ProductType 3))) 'a TIMEOUT with the host running is a failure'
    Assert-True ($null -eq (Get-ShellSurfaceAbsence -Open (New-ShellOpen 'INTERNAL') -Evidence $serverHostless)) 'any other code is a failure'

    $original = ((Get-Command Get-ShellSurfaceAbsence).ScriptBlock.ToString()) -replace "`r`n", "`n"
    $guard = ' -or $Evidence.ProductType -eq 1'
    Assert-True ($original.Contains($guard)) 'the invert target must be present in the real Get-ShellSurfaceAbsence'
    try {
        Set-LibFunctionText -ModuleName 'LibShell' -Name 'Get-ShellSurfaceAbsence' -Text $original.Replace($guard, '')
        $inverted = Get-ShellSurfaceAbsence -Open (New-ShellOpen 'TIMEOUT') -Evidence (New-ShellHostEvidence -HostRunning $false -ProductType 1)
        Assert-True ($null -ne $inverted) 'INVERT: without the workstation check, a workstation TIMEOUT must turn into a skip'
    } finally {
        Set-LibFunctionText -ModuleName 'LibShell' -Name 'Get-ShellSurfaceAbsence' -Text $original
    }
}

<#
    16. Resolve-ShellTrayAbsence skips only when the binary answers
        WINDOW_NOT_FOUND too. INVERT: dropping that check makes INTERNAL a
        skip.
#>
Invoke-SelfTest 'Resolve-ShellTrayAbsence skips only on WINDOW_NOT_FOUND and fails every other answer' {
    $missing = [pscustomobject]@{ Found = $false; Evidence = 'trayscan: promoted toolbars=0' }
    $found = [pscustomobject]@{ Found = $true; Evidence = 'trayscan: promoted toolbars=1' }
    $notFound = New-ShellOpen 'WINDOW_NOT_FOUND'
    Assert-True ($null -eq (Resolve-ShellTrayAbsence -Com $found -Snapshot $notFound)) 'a found toolbar is not an absence'
    Assert-True ((Resolve-ShellTrayAbsence -Com $missing -Snapshot $notFound -HostEvidence 'os') -like '*WINDOW_NOT_FOUND (os)*') 'no toolbar and WINDOW_NOT_FOUND is an absence carrying the evidence'
    $opened = [pscustomobject]@{ Opened = $true; ErrorCode = $null; WindowId = 'w-1' }
    Assert-Throws { Resolve-ShellTrayAbsence -Com $missing -Snapshot $opened } 'a tray the binary opens while the COM read finds none is a disagreement'
    foreach ($code in @('INTERNAL', 'TIMEOUT', 'PERM_DENIED')) {
        Assert-Throws { Resolve-ShellTrayAbsence -Com $missing -Snapshot (New-ShellOpen $code) } "$code is the binary's failure, not an absence"
    }

    $original = ((Get-Command Resolve-ShellTrayAbsence).ScriptBlock.ToString()) -replace "`r`n", "`n"
    $guard = "if (`$Snapshot.ErrorCode -ne 'WINDOW_NOT_FOUND') {"
    Assert-True ($original.Contains($guard)) 'the invert target must be present in the real Resolve-ShellTrayAbsence'
    try {
        Set-LibFunctionText -ModuleName 'LibShell' -Name 'Resolve-ShellTrayAbsence' -Text $original.Replace($guard, 'if ($false) {')
        Assert-NotThrows { Resolve-ShellTrayAbsence -Com $missing -Snapshot (New-ShellOpen 'INTERNAL') } 'INVERT: without the WINDOW_NOT_FOUND check, INTERNAL must turn into a skip'
    } finally {
        Set-LibFunctionText -ModuleName 'LibShell' -Name 'Resolve-ShellTrayAbsence' -Text $original
    }
}

<#
    17. Invoke-WithForegroundStolen runs the body only after a verified
        steal and always stops its contender. The native calls are replaced
        inside the module, so no window is opened. INVERT: dropping the
        refusal throw lets the body run with foreground still on the
        fixture.
#>
function Set-ForegroundStealMocks {
    param([bool]$Takes)
    $global:ForegroundMock = @{ Takes = $Takes; Stopped = 0; BodyRuns = 0 }
    & (Get-Module ForegroundContender) {
        function script:Start-ForegroundContender { return [pscustomobject]@{ Process = 'contender'; WindowHandle = [IntPtr]7 } }
        function script:Set-NativeForegroundWindow { param([IntPtr]$WindowHandle) return $global:ForegroundMock.Takes }
        function script:Set-NativeForegroundAwayFrom { param([IntPtr]$ExcludeHandle) return $null }
        function script:Get-NativeForegroundWindowHandle { if ($global:ForegroundMock.Takes) { return [IntPtr]7 } return [IntPtr]3 }
        function script:Stop-GuardedDetached { param($Handle) $global:ForegroundMock.Stopped++ }
    }
}

Invoke-SelfTest 'Invoke-WithForegroundStolen runs the body only after a verified steal and always stops the contender' {
    $modulePath = (Get-Module ForegroundContender).Path
    try {
        Set-ForegroundStealMocks -Takes $true
        Invoke-WithForegroundStolen -ExcludeHandle ([IntPtr]3) -Body { $global:ForegroundMock.BodyRuns++ }
        Assert-True ($global:ForegroundMock.BodyRuns -eq 1 -and $global:ForegroundMock.Stopped -eq 1) 'a verified steal runs the body once and stops the contender'

        Set-ForegroundStealMocks -Takes $false
        Assert-Throws { Invoke-WithForegroundStolen -ExcludeHandle ([IntPtr]3) -Body { $global:ForegroundMock.BodyRuns++ } } 'a refused steal must throw'
        Assert-True ($global:ForegroundMock.BodyRuns -eq 0 -and $global:ForegroundMock.Stopped -eq 1) 'a refused steal never runs the body and still stops the contender'

        Set-ForegroundStealMocks -Takes $true
        Assert-Throws { Invoke-WithForegroundStolen -ExcludeHandle ([IntPtr]3) -Body { throw 'body failed' } } 'a failing body propagates'
        Assert-True ($global:ForegroundMock.Stopped -eq 1) 'a failing body still stops the contender'

        $original = ((Get-Command Invoke-WithForegroundStolen).ScriptBlock.ToString()) -replace "`r`n", "`n"
        $guard = 'if (-not $taken -or $foreground -eq $ExcludeHandle) {'
        Assert-True ($original.Contains($guard)) 'the invert target must be present in the real Invoke-WithForegroundStolen'
        Set-ForegroundStealMocks -Takes $false
        Set-LibFunctionText -ModuleName 'ForegroundContender' -Name 'Invoke-WithForegroundStolen' -Text $original.Replace($guard, 'if ($false) {')
        Invoke-WithForegroundStolen -ExcludeHandle ([IntPtr]3) -Body { $global:ForegroundMock.BodyRuns++ }
        Assert-True ($global:ForegroundMock.BodyRuns -eq 1) 'INVERT: without the refusal throw, the body runs with foreground still on the fixture'
    } finally {
        Remove-Item -Path 'function:Invoke-WithForegroundStolen' -ErrorAction SilentlyContinue
        Import-Module $modulePath -Force -Global
        Remove-Variable -Name ForegroundMock -Scope Global -ErrorAction SilentlyContinue
    }
}

<#
    18. An aborted run prints the verdict recorded so far. A staged binary
        that does not exist aborts Run-E2E.ps1 before any desktop work.
        INVERT: removing the catch's Write-Verdict call leaves the abort
        with no VERDICT line.
#>
Invoke-SelfTest 'an aborted Run-E2E.ps1 prints the verdict recorded so far' {
    $missingBinary = New-TempPath -Prefix 'u7-missing-binary' -Extension '.exe'
    $runArgs = @('-NoProfile', '-File', $script:RunE2EPath, '-AgentDesktopBinary', $missingBinary)
    $aborted = Invoke-BoundedProcess -FilePath 'powershell.exe' -ArgumentList $runArgs -TimeoutSeconds 60
    Assert-True ($aborted.ExitCode -ne 0) "an aborted run must exit non-zero - saw $($aborted.ExitCode)"
    Assert-True ($aborted.StdOut -match '(?s)Run-E2E\.ps1: aborting.*VERDICT failed') "the abort must be followed by the verdict - stdout: $($aborted.StdOut.Trim())"

    $originalContent = Get-Content -LiteralPath $script:RunE2EPath -Raw
    $verdictLine = "    [void](Write-Verdict)`r`n"
    if (-not $originalContent.Contains($verdictLine)) { $verdictLine = "    [void](Write-Verdict)`n" }
    Assert-True ($originalContent.Contains($verdictLine)) 'the invert target line must be present in the real Run-E2E.ps1'
    try {
        Set-Content -LiteralPath $script:RunE2EPath -Value $originalContent.Replace($verdictLine, '') -NoNewline
        $inverted = Invoke-BoundedProcess -FilePath 'powershell.exe' -ArgumentList $runArgs -TimeoutSeconds 60
        Assert-True ($inverted.StdOut -notmatch 'VERDICT failed') 'INVERT: without the catch-path Write-Verdict, the abort prints no verdict'
    } finally {
        Set-Content -LiteralPath $script:RunE2EPath -Value $originalContent -NoNewline
    }
}

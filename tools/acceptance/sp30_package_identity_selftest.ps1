<#
.SYNOPSIS
  SP-30 package-identity selftest (synthetic only; self-contained).

.DESCRIPTION
  Builds small throwaway zips in %TEMP% from
  fixtures/acceptance/sp30/negative/build-info.*.json (four flat-layout exe
  stubs + one build-info variant, or no build-info for the nometa case) and
  asserts tools/acceptance/sp30_package_identity.ps1 accepts the control case
  and rejects every negative case (dirty tree, commit drift, armed package,
  missing metadata, zip/setup hash mismatch, missing SHA entry/file,
  live dirty tree via a temp git repo). Real packages under dist/ are never
  touched. No exe is launched, no port is bound (10808 never referenced), no
  proxy/registry/TUN/route/Run-key change. Temp files are removed afterwards.

  Each case runs the identity script in a child powershell process so the
  asserted exit code is the real process exit code. Exit 0 here means every
  case behaved as specified; exit 1 means at least one case did not.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp30_package_identity_selftest.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Identity = Join-Path $PSScriptRoot 'sp30_package_identity.ps1'
$NegDir = Join-Path $RepoRoot 'fixtures\acceptance\sp30\negative'
$Expected = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
$Work = Join-Path $env:TEMP ('sp30_selftest_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Work | Out-Null

$script:results = @()
$script:nFail = 0

function New-CaseZip([string]$Name, [string]$Variant) {
  $stage = Join-Path $Work ("stage_" + $Name)
  New-Item -ItemType Directory -Path $stage | Out-Null
  foreach ($exe in @('v2rayn_desktop.exe', 'net_host.exe', 'privileged_helper.exe', 'v2rayN-upgrade.exe')) {
    'stub' | Set-Content -LiteralPath (Join-Path $stage $exe) -Encoding ascii
  }
  if ($Variant -ne '__NONE__') {
    Copy-Item -LiteralPath (Join-Path $NegDir ("build-info." + $Variant + ".json")) -Destination (Join-Path $stage 'build-info.json')
  }
  $zip = Join-Path $Work ($Name + '.zip')
  Compress-Archive -LiteralPath $stage -DestinationPath $zip -Force
  return $zip
}

function Write-ShaFile([string]$Path, [string[]]$Lines) {
  $Lines | Set-Content -LiteralPath $Path -Encoding ascii
}

function Invoke-Identity([string]$ArgList) {
  $outFile = Join-Path $Work ('out_' + [guid]::NewGuid().ToString('N') + '.txt')
  $errFile = Join-Path $Work ('err_' + [guid]::NewGuid().ToString('N') + '.txt')
  # Launch the child with the SAME interpreter as the current host: starting
  # Windows PowerShell 5.1 from a pwsh 7 session inherits a PSModulePath that
  # 5.1 cannot resolve, which breaks utility cmdlets like Get-FileHash inside
  # the identity script (observed 2026-10-07). Real process exit codes are
  # still asserted.
  $hostExe = (Get-Process -Id $PID).Path
  if ([string]::IsNullOrWhiteSpace($hostExe)) { $hostExe = 'powershell' }
  $proc = Start-Process -FilePath $hostExe -ArgumentList ('-NoProfile -ExecutionPolicy Bypass -File "' + $Identity + '" ' + $ArgList) -NoNewWindow -Wait -PassThru -RedirectStandardOutput $outFile -RedirectStandardError $errFile
  $text = ''
  if (Test-Path -LiteralPath $outFile) { $text = (Get-Content -LiteralPath $outFile -Raw) }
  if (Test-Path -LiteralPath $errFile) { $text = $text + (Get-Content -LiteralPath $errFile -Raw) }
  return @{ exit = $proc.ExitCode; output = $text }
}

function Check([string]$Name, [int]$GotExit, [string]$Output, [int]$WantExit, [string]$WantSub) {
  $ok = ($GotExit -eq $WantExit)
  if ($WantSub -ne '' -and $Output -notmatch [regex]::Escape($WantSub)) { $ok = $false }
  if (-not $ok) { $script:nFail += 1 }
  if ($ok) { $st = 'pass' } else { $st = 'FAIL' }
  $script:results += [ordered]@{ case = $Name; status = $st; want_exit = $WantExit; got_exit = $GotExit; want_substring = $WantSub }
}

try {
  $good = New-CaseZip 'good' 'good'
  $goodSha = (Get-FileHash -LiteralPath $good -Algorithm SHA256).Hash.ToLower()
  $goodBase = [System.IO.Path]::GetFileName($good)

  $shaGood = Join-Path $Work 'good.SHA256SUMS'
  Write-ShaFile $shaGood @("$goodSha  $goodBase")

  $r = Invoke-Identity ('-Zip "' + $good + '" -ShaFile "' + $shaGood + '" -ExpectedCommit ' + $Expected)
  Check 'good-pass' $r.exit $r.output 0 'matches sha256sums'

  $r = Invoke-Identity ('-Zip "' + $good + '" -ExpectedCommit ' + $Expected)
  Check 'good-nosha-note' $r.exit $r.output 0 'sha pinning skipped'

  $dirty = New-CaseZip 'dirty' 'dirty'
  $sh = Join-Path $Work 'dirty.SHA256SUMS'
  Write-ShaFile $sh @("$((Get-FileHash -LiteralPath $dirty -Algorithm SHA256).Hash.ToLower())  $([System.IO.Path]::GetFileName($dirty))")
  $r = Invoke-Identity ('-Zip "' + $dirty + '" -ShaFile "' + $sh + '" -ExpectedCommit ' + $Expected)
  Check 'dirty-reject' $r.exit $r.output 1 'git_dirty != false'

  $drift = New-CaseZip 'drift' 'drift'
  $sh = Join-Path $Work 'drift.SHA256SUMS'
  Write-ShaFile $sh @("$((Get-FileHash -LiteralPath $drift -Algorithm SHA256).Hash.ToLower())  $([System.IO.Path]::GetFileName($drift))")
  $r = Invoke-Identity ('-Zip "' + $drift + '" -ShaFile "' + $sh + '" -ExpectedCommit ' + $Expected)
  Check 'drift-reject' $r.exit $r.output 1 '!= expected'

  $armed = New-CaseZip 'armed' 'armed'
  $sh = Join-Path $Work 'armed.SHA256SUMS'
  Write-ShaFile $sh @("$((Get-FileHash -LiteralPath $armed -Algorithm SHA256).Hash.ToLower())  $([System.IO.Path]::GetFileName($armed))")
  $r = Invoke-Identity ('-Zip "' + $armed + '" -ShaFile "' + $sh + '" -ExpectedCommit ' + $Expected)
  Check 'armed-reject' $r.exit $r.output 1 'smoke_armed != false'

  $nofield = New-CaseZip 'nofield' 'nofield'
  $sh = Join-Path $Work 'nofield.SHA256SUMS'
  Write-ShaFile $sh @("$((Get-FileHash -LiteralPath $nofield -Algorithm SHA256).Hash.ToLower())  $([System.IO.Path]::GetFileName($nofield))")
  $r = Invoke-Identity ('-Zip "' + $nofield + '" -ShaFile "' + $sh + '" -ExpectedCommit ' + $Expected)
  Check 'nofield-reject' $r.exit $r.output 1 'missing field: git_commit'

  $nometa = New-CaseZip 'nometa' '__NONE__'
  $sh = Join-Path $Work 'nometa.SHA256SUMS'
  Write-ShaFile $sh @("$((Get-FileHash -LiteralPath $nometa -Algorithm SHA256).Hash.ToLower())  $([System.IO.Path]::GetFileName($nometa))")
  $r = Invoke-Identity ('-Zip "' + $nometa + '" -ShaFile "' + $sh + '" -ExpectedCommit ' + $Expected)
  Check 'nometa-reject' $r.exit $r.output 1 'build-info.json not in zip'

  $sh = Join-Path $Work 'tamp.SHA256SUMS'
  Write-ShaFile $sh @("0000000000000000000000000000000000000000000000000000000000000000  $goodBase")
  $r = Invoke-Identity ('-Zip "' + $good + '" -ShaFile "' + $sh + '" -ExpectedCommit ' + $Expected)
  Check 'hashmismatch-reject' $r.exit $r.output 1 'mismatch'

  $sh = Join-Path $Work 'noentry.SHA256SUMS'
  Write-ShaFile $sh @("$goodSha  some-other-package.zip")
  $r = Invoke-Identity ('-Zip "' + $good + '" -ShaFile "' + $sh + '" -ExpectedCommit ' + $Expected)
  Check 'missingentry-reject' $r.exit $r.output 1 'entry missing'

  $r = Invoke-Identity ('-Zip "' + $good + '" -ShaFile "' + (Join-Path $Work 'does-not-exist.SHA256SUMS') + '" -ExpectedCommit ' + $Expected)
  Check 'noshafile-reject' $r.exit $r.output 1 'not found'

  $setup = Join-Path $Work 'synth-setup.exe'
  'setup-stub' | Set-Content -LiteralPath $setup -Encoding ascii
  $setupSha = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLower()
  $setupBase = [System.IO.Path]::GetFileName($setup)
  $sh = Join-Path $Work 'setupok.SHA256SUMS'
  Write-ShaFile $sh @("$goodSha  $goodBase", "$setupSha  $setupBase")
  $r = Invoke-Identity ('-Zip "' + $good + '" -ShaFile "' + $sh + '" -Setup "' + $setup + '" -ExpectedCommit ' + $Expected)
  Check 'setup-pass' $r.exit $r.output 0 'setup sha256 matches'
  $sh = Join-Path $Work 'setupbad.SHA256SUMS'
  Write-ShaFile $sh @("$goodSha  $goodBase", "1111111111111111111111111111111111111111111111111111111111111111  $setupBase")
  $r = Invoke-Identity ('-Zip "' + $good + '" -ShaFile "' + $sh + '" -Setup "' + $setup + '" -ExpectedCommit ' + $Expected)
  Check 'setup-mismatch-reject' $r.exit $r.output 1 'setup sha256 mismatch'

  $gitHere = ($null -ne (Get-Command git -ErrorAction SilentlyContinue))
  if ($gitHere) {
    $fakeRepo = Join-Path $Work 'fakerepo'
    New-Item -ItemType Directory -Path $fakeRepo | Out-Null
    & git -C $fakeRepo init -q 2>$null
    & git -C $fakeRepo -c user.email=selftest@localhost -c user.name=selftest commit --allow-empty -m init -q 2>$null
    $r = Invoke-Identity ('-Zip "' + $good + '" -ShaFile "' + $shaGood + '" -ExpectedCommit ' + $Expected + ' -RequireCleanTree -RepoRootOverride "' + $fakeRepo + '"')
    Check 'cleantree-clean-pass' $r.exit $r.output 0 'working tree clean'
    'x' | Set-Content -LiteralPath (Join-Path $fakeRepo 'untracked.txt') -Encoding ascii
    $r = Invoke-Identity ('-Zip "' + $good + '" -ShaFile "' + $shaGood + '" -ExpectedCommit ' + $Expected + ' -RequireCleanTree -RepoRootOverride "' + $fakeRepo + '"')
    Check 'cleantree-dirty-reject' $r.exit $r.output 1 'working tree dirty'
  } else {
    $script:results += [ordered]@{ case = 'cleantree-*'; status = 'skip'; want_exit = -1; got_exit = -1; want_substring = 'git not on PATH' }
  }

  $r = Invoke-Identity '-VerifyFixture'
  Check 'fixture-shape-pass' $r.exit $r.output 0 'fixture_lines'
}
finally {
  Remove-Item -LiteralPath $Work -Recurse -Force -ErrorAction SilentlyContinue
}

if ($script:nFail -gt 0) { $verdict = 'fail' } else { $verdict = 'pass' }
$summary = [ordered]@{
  mode = 'identity-selftest'; status = $verdict; failed = $script:nFail
  total = $script:results.Count; cases = $script:results
  side_effects = 'none (temp dir removed; no exe launch; no port bind; 10808 untouched; no proxy/registry/TUN/Run-key change)'
}
($summary | ConvertTo-Json -Depth 5) | Write-Output
if ($script:nFail -gt 0) { exit 1 } else { exit 0 }

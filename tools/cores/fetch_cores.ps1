<#
.SYNOPSIS
  Download official windows-amd64 core assets, verify optional upstream
  checksums, extract, normalize the executable name, and emit a lock fragment.

  Uses the current system/environment proxy as an ordinary client. Never
  touches proxy configuration, the host system proxy, routing, or TUN.

.NOTES
  Downloaded binaries/zips are git-ignored (tools/cores/**). The emitted
  cores.lock.generated.json is a candidate fragment for review; it is not
  copied into cores.lock.json automatically.
#>
[CmdletBinding()]
param(
  [string[]]$Only,
  [switch]$Force
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$CoresRoot = $PSScriptRoot

$Manifest = @(
  [pscustomobject]@{ core='v2fly';      dir='v2fly';      version='v4.45.2';          tag='v4.45.2';           repo='v2fly/v2ray-core';         asset='v2ray-windows-64.zip';                    kind='zip'; inner='v2ray.exe';                   target='v2ray.exe';                   checksum='dgst' },
  [pscustomobject]@{ core='v2fly_v5';   dir='v2fly';      version='v5.53.0';          tag='v5.53.0';           repo='v2fly/v2ray-core';         asset='v2ray-windows-64.zip';                    kind='zip'; inner='v2ray.exe';                   target='v2ray.exe';                   checksum='dgst' },
  [pscustomobject]@{ core='mihomo';     dir='mihomo';     version='v1.19.32';         tag='v1.19.32';          repo='MetaCubeX/mihomo';         asset='mihomo-windows-amd64-v1-v1.19.32.zip';    kind='zip'; inner='mihomo-windows-amd64.exe';    target='mihomo-windows-amd64-v1.exe'; checksum=$null },
  [pscustomobject]@{ core='hysteria';   dir='hysteria';   version='v1.3.5';           tag='v1.3.5';            repo='apernet/hysteria';         asset='hysteria-windows-amd64.exe';              kind='exe'; inner=$null;                         target='hysteria.exe';                checksum=$null },
  [pscustomobject]@{ core='naiveproxy'; dir='naiveproxy'; version='v154.0.8037.49-2'; tag='v154.0.8037.49-2';  repo='klzgrad/naiveproxy';       asset='naiveproxy-v154.0.8037.49-2-win-x64.zip'; kind='zip'; inner='naive.exe';                   target='naive.exe';                   checksum=$null },
  [pscustomobject]@{ core='tuic';       dir='tuic';       version='v1.0.0';           tag='tuic-client-1.0.0'; repo='EAimTY/tuic';              asset='tuic-client-1.0.0-x86_64-pc-windows-msvc.exe'; kind='exe'; inner=$null;                    target='tuic-client.exe';             checksum='sha256sum' },
  [pscustomobject]@{ core='juicity';    dir='juicity';    version='v0.5.0';           tag='v0.5.0';            repo='juicity/juicity';          asset='juicity-windows-x86_64.zip';              kind='zip'; inner='juicity-client.exe';          target='juicity-client.exe';          checksum='dgst' },
  [pscustomobject]@{ core='hysteria2';  dir='hysteria';   version='v2.12.3';          tag='app/v2.12.3';       repo='apernet/hysteria';         asset='hysteria-windows-amd64.exe';              kind='exe'; inner=$null;                         target='hysteria-windows-amd64.exe';  checksum=$null },
  [pscustomobject]@{ core='brook';      dir='brook';      version='v20270101';        tag='v20270101';         repo='txthinking/brook';         asset='brook_windows_amd64.exe';                 kind='exe'; inner=$null;                         target='brook_windows_amd64.exe';     checksum=$null },
  [pscustomobject]@{ core='overtls';    dir='overtls';    version='v0.3.15';          tag='v0.3.15';           repo='ShadowsocksR-Live/overtls'; asset='overtls-x86_64-pc-windows-msvc.zip';     kind='zip'; inner='overtls-bin.exe';             target='overtls-bin.exe';             checksum=$null },
  [pscustomobject]@{ core='shadowquic'; dir='shadowquic'; version='v0.4.0';           tag='v0.4.0';            repo='spongebob888/shadowquic';  asset='shadowquic-x86_64-windows.exe';           kind='exe'; inner=$null;                         target='shadowquic.exe';              checksum=$null },
  [pscustomobject]@{ core='mieru';      dir='mieru';      version='v3.38.0';          tag='v3.38.0';           repo='enfein/mieru';             asset='mieru_3.38.0_windows_amd64.zip';          kind='zip'; inner='mieru.exe';                   target='mieru.exe';                   checksum='sha256.txt' }
)

function Get-Sha256([string]$Path) {
  (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-RemoteText([string]$Url) {
  (Invoke-WebRequest -Uri $Url -TimeoutSec 300 -UseBasicParsing).Content
}

function Resolve-Checksum([string]$Core, [string]$Version, [string]$Asset, [string]$ChecksumKind, [string]$AssetPath) {
  $local = Get-Sha256 $AssetPath
  $result = [ordered]@{ sha256 = $local; sha256_verified = $false; reported = $null }
  if (-not $ChecksumKind) { return $result }
  $url = $null
  switch ($ChecksumKind) {
    'dgst'      { $url = "https://github.com/$($script:Current.repo)/releases/download/$($script:Current.tag)/$Asset.dgst" }
    'sha256sum' { $url = "https://github.com/$($script:Current.repo)/releases/download/$($script:Current.tag)/$Asset.sha256sum" }
    'sha256.txt'{ $url = "https://github.com/$($script:Current.repo)/releases/download/$($script:Current.tag)/$Asset.sha256.txt" }
  }
  try {
    $text = Get-RemoteText $url
    $m = [regex]::Match($text, '(?im)([0-9a-f]{64})')
    if ($m.Success) {
      $reported = $m.Groups[1].Value.ToLowerInvariant()
      $result.reported = $reported
      $result.sha256_verified = ($reported -eq $local)
    }
  } catch {
    $result.reported = "checksum fetch failed: $($_.Exception.Message)"
  }
  return $result
}

$selected = if ($Only) { $Manifest | Where-Object { $Only -contains $_.core } } else { $Manifest }
$fragment = [System.Collections.Generic.List[object]]::new()

foreach ($item in $selected) {
  $script:Current = $item
  $versionDir = Join-Path (Join-Path $CoresRoot $item.dir) $item.version
  New-Item -ItemType Directory -Force -Path $versionDir | Out-Null
  $assetPath = Join-Path $versionDir $item.asset
  $url = "https://github.com/$($item.repo)/releases/download/$($item.tag)/$($item.asset)"

  Write-Output "==> $($item.core) $($item.version): $($item.asset)"
  if ((Test-Path -LiteralPath $assetPath) -and -not $Force) {
    Write-Output "    asset already present, hashing"
  } else {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    Invoke-WebRequest -Uri $url -OutFile $assetPath -TimeoutSec 600 -UseBasicParsing
    Write-Output ("    downloaded {0:N0} bytes in {1:N1}s" -f (Get-Item -LiteralPath $assetPath).Length, $sw.Elapsed.TotalSeconds)
  }

  if ($item.kind -eq 'zip') {
    $extractDir = $versionDir
    Expand-Archive -LiteralPath $assetPath -DestinationPath $extractDir -Force
    $targetPath = Join-Path $versionDir $item.target
    if (-not (Test-Path -LiteralPath $targetPath)) {
      $found = Get-ChildItem -LiteralPath $versionDir -Recurse -File -Filter $item.inner | Select-Object -First 1
      if (-not $found) { throw "inner executable '$($item.inner)' not found after extracting $($item.asset)" }
      Copy-Item -LiteralPath $found.FullName -Destination $targetPath -Force
    }
  } else {
    $targetPath = Join-Path $versionDir $item.target
    if ($item.target -ne $item.asset) {
      Copy-Item -LiteralPath $assetPath -Destination $targetPath -Force
    }
  }

  $checksum = Resolve-Checksum $item.core $item.version $item.asset $item.checksum $assetPath
  $entry = [ordered]@{
    core                 = $item.core
    release_tag          = $item.tag
    core_version         = ($item.version -replace '^v', '')
    asset                = $item.asset
    asset_size_bytes     = (Get-Item -LiteralPath $assetPath).Length
    source               = $url
    sha256               = $checksum.sha256
    sha256_verified      = $checksum.sha256_verified
    extracted_to         = ("tools/cores/{0}/{1}/" -f $item.dir, $item.version)
    executable           = ("tools/cores/{0}/{1}/{2}" -f $item.dir, $item.version, $item.target)
    executable_sha256    = (Get-Sha256 $targetPath)
    platform             = 'windows-amd64'
  }
  if ($checksum.reported) { $entry['sha256_reported'] = $checksum.reported }
  $fragment.Add([pscustomobject]$entry)
  $v = if ($checksum.sha256_verified) { 'verified' } else { 'local' }
  Write-Output ("    sha256=$($checksum.sha256) ($v)")
}

$null = New-Item -ItemType Directory -Force -Path (Join-Path $CoresRoot 'logs')
$out = Join-Path $CoresRoot 'cores.lock.generated.json'
$fragment | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $out -Encoding utf8
Write-Output "wrote $out"

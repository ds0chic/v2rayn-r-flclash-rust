[CmdletBinding()]
param(
    [string]$Xray = 'C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn\tools\cores\xray\v26.3.27\xray.exe',
    [string]$Work = 'C:\Users\Colby\AppData\Local\Temp\opencode\kcp-probe2'
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $Work | Out-Null

function New-Cfg {
    param($Udp)
    $stream = [ordered]@{
        network     = 'kcp'
        kcpSettings = [ordered]@{ mtu = 1350; tti = 20; uplinkCapacity = 5; downlinkCapacity = 20; cwndMultiplier = 1; maxSendingWindow = 0 }
    }
    if ($null -ne $Udp) { $stream['finalmask'] = [ordered]@{ udp = $Udp } }
    $cfg = [ordered]@{
        log      = @{ loglevel = 'warning' }
        inbounds = @(@{ tag = 'socks'; port = 11891; protocol = 'socks'; listen = '127.0.0.1'; settings = @{ auth = 'noauth' } })
        outbounds = @(
            [ordered]@{
                tag = 'proxy'; protocol = 'vless'
                settings = @{ address = '192.0.2.10'; port = 443; id = '11111111-2222-3333-4444-555555555555'; encryption = 'none' }
                streamSettings = $stream
            },
            @{ protocol = 'freedom'; tag = 'direct' },
            @{ protocol = 'blackhole'; tag = 'block' }
        )
    }
    return ($cfg | ConvertTo-Json -Depth 20)
}
function M { param([string]$Type, $Settings) 
    if ($null -eq $Settings) { return @{ type = $Type } }
    return @{ type = $Type; settings = $Settings }
}

$variants = [ordered]@{
    'P1-type-wechat'              = @( (M 'wechat' $null) )
    'P2-type-srtp'                = @( (M 'srtp' $null) )
    'P3-type-utp'                 = @( (M 'utp' $null) )
    'P4-type-dtls'                = @( (M 'dtls' $null) )
    'P5-type-wireguard'           = @( (M 'wireguard' $null) )
    'P6-type-dns'                 = @( (M 'dns' $null) )
    'P7-header-wechat'            = @( (M 'header-wechat' $null) )
    'P8-header-srtp'              = @( (M 'header-srtp' $null) )
    'P9-original+wechat'          = @( (M 'mkcp-original' $null), (M 'wechat' $null) )
    'P10-original+wechat+value'   = @( (M 'mkcp-original' @{ value = 'seed' }), (M 'wechat' $null) )
    'P11-wechat+value'            = @( (M 'wechat' @{ value = 'seed' }) )
    'P12-header-original'         = @( (M 'header-original' $null) )
    'P13-mkcp-legacy-empty'       = @( (M 'mkcp-legacy' @{}) )
    'P14-mkcp-original-header'    = @( (M 'mkcp-original' @{ header = 'wechat' }) )
}

$report = @()
foreach ($name in $variants.Keys) {
    $file = Join-Path $Work ("$name.json")
    New-Cfg -Udp $variants[$name] | Set-Content -LiteralPath $file -Encoding utf8
    $out = & $Xray run -test -config $file 2>&1
    $exit = $LASTEXITCODE
    $text = ($out | Out-String).Trim()
    if ($text.Length -gt 400) { $text = $text.Substring($text.Length - 400) }
    $report += [pscustomobject]@{ variant = $name; exit = $exit; tail = $text }
    Write-Host ("[{0}] {1} exit={2}" -f $(if ($exit -eq 0) { 'PASS' } else { 'FAIL' }), $name, $exit)
}
$report | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Work 'probe-results.json') -Encoding utf8
Write-Output "---- report ----"
$report | ForEach-Object { "### $($_.variant) exit=$($_.exit)`n$($_.tail)`n" }

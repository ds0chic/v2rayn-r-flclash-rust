[CmdletBinding()]
param(
    [string]$Xray = 'C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn\tools\cores\xray\v26.3.27\xray.exe',
    [string]$Work = 'C:\Users\Colby\AppData\Local\Temp\opencode\kcp-probe'
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
        inbounds = @(@{ tag = 'socks'; port = 11890; protocol = 'socks'; listen = '127.0.0.1'; settings = @{ auth = 'noauth' } })
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
    'A-current-legacy'         = @( (M 'mkcp-legacy' @{ value = 'seed' }), (M 'mkcp-legacy' @{ header = 'wechat' }) )
    'B-aes128gcm-header-value' = @( (M 'mkcp-aes128gcm' @{ value = 'seed' }), (M 'mkcp-aes128gcm' @{ header = 'wechat' }) )
    'C-aes128gcm-header-only'  = @( (M 'mkcp-aes128gcm' @{ header = 'wechat' }) )
    'D-aes128gcm-value-only'   = @( (M 'mkcp-aes128gcm' @{ value = 'seed' }) )
    'E-aes128gcm-no-settings'  = @( (M 'mkcp-aes128gcm' $null) )
    'F-original-header-value'  = @( (M 'mkcp-original' @{ value = 'seed' }), (M 'mkcp-original' @{ header = 'wechat' }) )
    'G-original-header-only'   = @( (M 'mkcp-original' @{ header = 'wechat' }) )
    'H-original-value-only'    = @( (M 'mkcp-original' @{ value = 'seed' }) )
    'I-original-no-settings'   = @( (M 'mkcp-original' $null) )
    'J-bare-mkcp'              = @( (M 'mkcp' @{ header = 'wechat' }) )
    'K-empty-udp'              = @()
    'L-omit-finalmask'         = $null
    'M-aes128gcm-value0'       = @( (M 'mkcp-aes128gcm' @{ value = 0 }) )
    'N-aes128gcm-value-num'    = @( (M 'mkcp-aes128gcm' @{ value = 12345 }) )
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
    Write-Host ("[{0}] {1} exit={2}" -f $(if ($exit -eq 0) { 'PASS' } ELSE { 'FAIL' }), $name, $exit)
}
$report | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Work 'probe-results.json') -Encoding utf8
Write-Output "---- report ----"
$report | ForEach-Object { "### $($_.variant) exit=$($_.exit)`n$($_.tail)`n" }

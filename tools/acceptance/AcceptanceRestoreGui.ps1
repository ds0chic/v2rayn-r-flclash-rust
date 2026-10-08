<#
.SYNOPSIS
  One-click acceptance restore launcher (simple GUI, test-tool layer only).

.DESCRIPTION
  Two buttons for a manual acceptance run:
    1. 准备测试（保存快照） - read-only baseline capture into the current user's
       local store.
    2. 一键还原（恢复测试前状态） - idempotent, fail-closed rollback of the
       recorded categories only. If the baseline/current state references
       127.0.0.1:20808 the restore is REFUSED with a message pointing at an
       isolated-VM checkpoint.

  It never connects to / probes / binds / stops 127.0.0.1:20808, and it never
  resets the whole network or deletes routes/registry entries blindly.
#>
[CmdletBinding()]
param([string]$StoreDir = '')

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
. (Join-Path $PSScriptRoot 'AcceptanceRestore.ps1')

$script:store = Get-AcceptanceStore -StoreDir $StoreDir

$form = New-Object System.Windows.Forms.Form
$form.Text = 'v2rayn-r 验收还原（测试工具）'
$form.Size = New-Object System.Drawing.Size(620, 420)
$form.StartPosition = 'CenterScreen'

$lbl = New-Object System.Windows.Forms.Label
$lbl.Location = New-Object System.Drawing.Point(16, 12)
$lbl.Size = New-Object System.Drawing.Size(580, 260)
$lbl.Font = New-Object System.Drawing.Font('Consolas', 9)
$form.Controls.Add($lbl)

function Update-Status {
  $valid = Test-AcceptanceSnapshot -StoreDir $script:store
  $lines = @()
  $lines += "快照目录: $script:store"
  if ($valid.ok) {
    $lines += "快照状态: 有效"
    $lines += "20808 保护: $(if ($valid.protected20808) { '是（还原将拒绝）' } else { '否' })"
    $lines += "可恢复类别: winInetProxy（仅在值与基线不同且不含 20808 时）"
    $lines += "不可恢复类别: Run-key 删除、路由/TUN、全局网络重置 -> 需隔离 VM checkpoint"
  } else {
    $lines += "快照状态: 无效（$($valid.reason)）"
    $lines += "请先点『准备测试』保存基线。"
  }
  $lines += ''
  $lines += '流程: 点「准备测试」-> 手动验收 -> 点「一键还原」。'
  $lines += '安全: 不连接/探测/绑定 20808；不盲删路由/注册表；日志不打印代理地址与凭据。'
  $lbl.Text = ($lines -join "`r`n")
}

$btnSnap = New-Object System.Windows.Forms.Button
$btnSnap.Text = '1. 准备测试（保存快照）'
$btnSnap.Location = New-Object System.Drawing.Point(16, 290)
$btnSnap.Size = New-Object System.Drawing.Size(190, 40)
$btnSnap.Add_Click({
  try {
    $out = New-AcceptanceSnapshot -Backend real -StoreDir $script:store
    [System.Windows.Forms.MessageBox]::Show("快照已保存。`r`n$out", '准备测试') | Out-Null
  } catch {
    [System.Windows.Forms.MessageBox]::Show("保存失败: $_", '准备测试') | Out-Null
  }
  Update-Status
})
$form.Controls.Add($btnSnap)

$btnRestore = New-Object System.Windows.Forms.Button
$btnRestore.Text = '2. 一键还原（恢复测试前状态）'
$btnRestore.Location = New-Object System.Drawing.Point(216, 290)
$btnRestore.Size = New-Object System.Drawing.Size(210, 40)
$btnRestore.Add_Click({
  $plan = Get-AcceptanceRestorePlan -Backend real -StoreDir $script:store
  if ($plan.refused) {
    [System.Windows.Forms.MessageBox]::Show("已拒绝（fail-closed）: $($plan.reason)`r`n`r`n请在隔离 VM 中用 checkpoint 恢复。", '一键还原') | Out-Null
    return
  }
  $lines = ($plan.items | ForEach-Object { "- $($_.category): $($_.action) ($($_.reason))" }) -join "`r`n"
  $answer = [System.Windows.Forms.MessageBox]::Show("将按以下类别恢复（仅本次记录项）:`r`n$lines`r`n`r`n继续？", '确认还原',
    [System.Windows.Forms.MessageBoxButtons]::YesNo)
  if ($answer -ne [System.Windows.Forms.DialogResult]::Yes) { return }
  $res = Restore-AcceptanceSnapshot -Backend real -StoreDir $script:store
  [System.Windows.Forms.MessageBox]::Show("还原完成: ok=$($res.ok) refused=$($res.refused)", '一键还原') | Out-Null
  Update-Status
})
$form.Controls.Add($btnRestore)

$btnOpen = New-Object System.Windows.Forms.Button
$btnOpen.Text = '打开快照目录'
$btnOpen.Location = New-Object System.Drawing.Point(436, 290)
$btnOpen.Size = New-Object System.Drawing.Size(150, 40)
$btnOpen.Add_Click({
  if (Test-Path -LiteralPath $script:store) { Start-Process explorer.exe $script:store | Out-Null }
})
$form.Controls.Add($btnOpen)

Update-Status
[void]$form.ShowDialog()

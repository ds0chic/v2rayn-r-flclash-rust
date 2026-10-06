<#
.SYNOPSIS
  SP-30 隔离数据目录准备（默认 dry-run 只打印计划；执行态仅建空目录）。

.DESCRIPTION
  为「首次使用→备份重开」验收准备隔离数据目录：
    * 默认 -DryRun：只打印计划（目录位置、环境变量、清理规则），不写文件系统。
    * 去掉 -DryRun：仅在 %TEMP%\sp30_accept_<guid> 下新建空目录并写入
      README.txt（记录基线 commit + 用途），返回路径。绝不触碰真实用户数据
      目录、注册表、代理设置、10808 端口。
  验收运行时：V2RAYN_R_DATA_DIR=<本脚本输出目录> 启动候选包普通入口
  （v2rayn_desktop.exe 双击路径），退出后同目录重开即完成 S1/S6。

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp30_prepare_datadir.ps1 -DryRun
#>
[CmdletBinding()]
param([switch]$DryRun)

$ErrorActionPreference = 'Stop'
$plan = [ordered]@{
  mode = 'datadir-plan'; status = 'identified'
  location_pattern = '%TEMP%\sp30_accept_<guid>'
  env = 'V2RAYN_R_DATA_DIR=<new dir>（验收进程独占；退出即重开同一目录）'
  writes_when_armed = @('新建空目录', '目录内 README.txt（基线+用途）')
  never = @('用户真实数据目录', '注册表/系统代理', '10808', 'TUN/路由/DNS/Run-key')
  cleanup = '验收后删除本脚本创建的目录树（owned 资源）；失败保留供取证并记录路径'
}
($plan | ConvertTo-Json -Depth 4) | Write-Output
if ($DryRun) { exit 0 }

$dir = Join-Path $env:TEMP ('sp30_accept_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $dir | Out-Null
'SP-30 acceptance data dir. Baseline 3635392. Synthetic only. Delete after run.' |
  Set-Content -LiteralPath (Join-Path $dir 'README.txt') -Encoding utf8
([ordered]@{ data_dir = $dir } | ConvertTo-Json) | Write-Output
exit 0

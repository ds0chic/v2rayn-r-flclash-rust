# SP-30 手工验收 Runbook（候选包 828b785：S3 / S5 / S6 / S7）

> 范围：仅覆盖 README 中 `blocked` 的人工点击流步骤（S3 合成订阅导入、S5 备份、
> S6 停止+重开持久性、S7 恢复）。S0/S1/S2/S4 已有自动化证据，不在本 runbook 内。
>
> 候选包：`dist/v2rayN-R-1.0.0+1-windows-x64.zip`（commit `828b785`，
> `git_dirty=false`、`smoke_armed=false`，见 `runs/20261007-candidate-828b785/s0-identity.json`）。
>
> 约束：合成数据 only；不碰 10808 / 系统代理 / TUN / 路由 / Run-key；
> 只停止本 runbook 启动的 owned PID 树；不 commit。
>
> 图例：`[人工]` = 人在 GUI 里点；`[命令]` = 人在 pwsh 里粘贴执行（只读优先，
> 备份写操作除外）。所有命令均为复制粘贴可执行，按顺序跑。

## 0. 前置与约定

### 0.1 证据落盘根目录

```powershell
$RunRoot = "docs\evidence\stable-port\SP-30\runs\20261007-candidate-828b785\manual"
New-Item -ItemType Directory -Path $RunRoot -Force | Out-Null
$RunRoot
```

- 本 runbook 引用的截图/清单全部落到 `$RunRoot` 下：
  `s3/`、`s5/`、`s6/`、`s7/` 四个子目录（下面各节会建）。
- 每个截图文件名固定，登记到 `observations.json`（手工补记，格式见 README
  通用记录格式：`step/entry/expected/actual/exit/evidence/cleanup`）。

### 0.2 夹具（已验证存在）

- `fixtures/acceptance/sp30/synthetic-sub.txt` —— **存在**（4 行：
  3×vmess + 1×ss；remarks `sp30-synth-01/02/03`、`sp30-synth-ss`；
  地址全在 `192.0.2.0/24`，端口 11881–11884）。
- 只读使用；**不要**改它，不要把真实订阅/凭据粘进去。

```powershell
# [命令] 夹具形状复核（只读，不触网、不写库）
powershell -NoProfile -ExecutionPolicy Bypass -File tools\acceptance\sp30_package_identity.ps1 -VerifyFixture
Get-Content -LiteralPath "fixtures\acceptance\sp30\synthetic-sub.txt"
```

预期：`VerifyFixture` 通过；文件恰 4 非空行。

### 0.3 UI 入口名称（按源码实际写法，本 runbook 统一用这些名字）

| 位置 | 名称（照字面点） |
|---|---|
| 顶层菜单 | `配置项`、`订阅分组`、`设置`、`帮助`、`重启服务`、`关闭` |
| `配置项` 子菜单 | `从剪贴板导入分享链接`（`Ctrl+V`） |
| `订阅分组` 子菜单 | `订阅分组设置`（窗口标题同名） |
| `设置` 子菜单 | `备份和还原`（窗口标题为 `备份与还原`，注意“和/与”差异） |
| 节点表右键菜单 | `设为活动`（S4 用，本 runbook S3 后可选做，不强制） |
| 导入粘贴对话框 | 标题 `从文本导入节点`，按钮 `导入` / `取消` |
| 导入确认对话框 | 文案含 `将导入 N 个节点到…，一次提交。`，按钮 `提交导入` |
| 备份窗口 | 标题 `备份与还原`；分区 `本地备份 / 恢复`；按钮 `本地备份`、`选择备份 ZIP 恢复`、`选择备份包目录恢复`；底部状态条（`就绪` / `处理中…` / 结果文案）；`关闭` |

> 不精确点（已标出，见 §5）：README S3 写“从文件导入”，但源码里
> 分享链接导入只有剪贴板入口（`importFromClipboard`）+ 粘贴文本对话框回退
> （`importFromTextDialog`），**没有**“从文件选择 fixture”的菜单项。
> 下面 S3 用“打开 fixture → 全选复制 → `从剪贴板导入分享链接`”的等效路径，
> 与源码一致。

### 0.4 每次启动通用的提取/启动方式

候选包是扁平 portable zip（见 `r4_32_real_entry.ps1` 第 1 阶段），**每次都用
新鲜隔离目录**，环境变量只传 `V2RAYN_R_DATA_DIR`（外加窗口钩子变量仅在注明时用）：

```powershell
# [命令] 解压候选包到隔离区（只做一次，后面各节复用 $Pkg）
$WorkRoot   = Join-Path $env:TEMP ("sp30_manual_" + [guid]::NewGuid().ToString("N"))
$ExtractDir = Join-Path $WorkRoot "extract"
New-Item -ItemType Directory -Path $ExtractDir -Force | Out-Null
Expand-Archive -LiteralPath "dist\v2rayN-R-1.0.0+1-windows-x64.zip" -DestinationPath $ExtractDir -Force
$Pkg    = (Get-ChildItem -LiteralPath $ExtractDir -Directory | Select-Object -First 1).FullName
$MainExe = Join-Path $Pkg "v2rayn_desktop.exe"
$MainExe; Test-Path -LiteralPath $MainExe
```

```powershell
# [命令] 新建本轮隔离数据目录（S3–S7 全程共用同一个，不要中途换）
$DataDir = Join-Path $WorkRoot "data_manual"
New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
$DataDir; (Get-ChildItem -LiteralPath $DataDir -Force | Measure-Object).Count  # 预期 0
```

```powershell
# [命令] 启动（只设 V2RAYN_R_DATA_DIR；不设 AUTO_SMOKE；不动 10808/代理/TUN）
$env:V2RAYN_R_DATA_DIR = $DataDir
Start-Process -FilePath $MainExe -WorkingDirectory $Pkg
$env:V2RAYN_R_DATA_DIR = $null
```

- 启动后记下 PID（下面各节的停止命令要用）：

```powershell
# [命令] 记录你启动的 PID（在任务管理器/详情里按路径核对后填入）
$AppPid = <把主窗口进程 PID 填在这里>
$AppPid
```

- 停止时**只停这个 PID 树**（与 `r4_32_real_entry.ps1` 的 `Stop-OwnedTree` 同语义，
  禁止按名批杀）：

```powershell
# [命令] 只停 owned PID 树（$AppPid 必须是上面记录的那个）
$queue = [System.Collections.Generic.Queue[int]]::new(); $queue.Enqueue($AppPid)
$owned = @($AppPid)
while ($queue.Count -gt 0) {
  $p = $queue.Dequeue()
  foreach ($c in (Get-CimInstance Win32_Process -Filter "ParentProcessId=$p" -ErrorAction SilentlyContinue)) {
    $owned += [int]$c.ProcessId; $queue.Enqueue([int]$c.ProcessId)
  }
}
$owned = @($owned | Sort-Object -Unique); [array]::Reverse($owned)
foreach ($id in $owned) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
```

---

## 1. S3 —— 合成订阅导入 `[人工为主]`

目标：4 节点入库，remarks=`sp30-synth-01/02/03` + `sp30-synth-ss`；
失败行不半写（事务语义）。

### 1.1 先建证据目录 + 基线快照 `[命令]`

```powershell
# [命令]
New-Item -ItemType Directory -Path (Join-Path $RunRoot "s3") -Force | Out-Null
Get-ChildItem -LiteralPath $DataDir -Force | Format-Table Name, Length -AutoSize
```

### 1.2 点击流 `[人工]`

1. 按 §0.4 启动 app，等主窗口出现（参考既有证据截图
   `runs/20261007-candidate-828b785/real-entry/r4-32-first-run.png` 的窗口样子）。
2. 另开一个记事本，用**只读**方式打开
   `fixtures\acceptance\sp30\synthetic-sub.txt`，`Ctrl+A` → `Ctrl+C`
   （复制全部 4 行；不要改文件，不要另存）。
3. 回到 app 主窗口 → 顶层菜单 **`配置项`** → **`从剪贴板导入分享链接`**。
4. 观察导入预览/确认对话框：
   - 若弹出确认框（文案形如 `将导入 4 个节点到…，一次提交。`）→ 点 **`提交导入`**。
   - 若报 `剪贴板为空…` → 说明复制没带过去，重做步骤 2（回到 app 前不要再碰剪贴板），再点一次菜单。
   - 若报 `导入失败：…`（分类错误）→ **停**，按 §1.4 取证（这是失败证据，不要重试覆盖现场）。
5. 若存在“作为订阅添加”之类的岔路对话框（payload 被识别成订阅 URL 时才会出；
   本 fixture 是分享链接，**不应**出现）→ 选取消/关闭并记录，所见即所得记入实际值。
6. 回到主窗口节点表，肉眼核对出现 4 行，remarks 列为
   `sp30-synth-01`、`sp30-synth-02`、`sp30-synth-03`、`sp30-synth-ss`。

### 1.3 截图 `[人工]`

- `s3/s3-import-confirm.png` —— 确认对话框（含“将导入 4 个节点”文案）。
- `s3/s3-table-after-import.png` —— 主窗口节点表 4 行全可见（含 remarks 列）。
- 若有错误弹窗：`s3/s3-import-error.png`。

保存到：`docs/evidence/stable-port/SP-30/runs/20261007-candidate-828b785/manual/s3/`。

### 1.4 点击后验证 `[命令]`（只读：文件清单 + DB 行数，不写库）

```powershell
# [命令] 数据目录清单（预期新增 guiNDB.db；注意文件名大小写/拼写见下）
Get-ChildItem -LiteralPath $DataDir -Force | Format-Table Name, Length -AutoSize
Test-Path -LiteralPath (Join-Path $DataDir "guiNDB.db")
Test-Path -LiteralPath (Join-Path $DataDir "guiNConfig.json")
```

> DB 文件名注意：`r4_32_real_entry.ps1` 用 `guiNDB.db`，
> `smoke_windows.ps1` 里轮询的是 `guiNNDB.db`（多一个 N，疑似笔误）。
> 以 `guiNDB.db` 为准，若两者都存在/都不存在，**如实记录**，不要改名凑数。

```powershell
# [命令] DB 节点清单 diff（需要本机有 sqlite3；若没有则跳过并记为未验证，见 §5）
# 有 sqlite3 时：
sqlite3 (Join-Path $DataDir "guiNDB.db") ".tables"
sqlite3 (Join-Path $DataDir "guiNDB.db") "SELECT remarks FROM profiles ORDER BY remarks;"
```

- 通过判定：表里恰 4 行，remarks 与上面一致；失败行无半写（行数不是 1/2/3）。
- 不符即停：把 `SELECT` 输出存 `s3/s3-db-profiles.txt`，记 `blocked` 登记，不重放导入。

---

## 2. S5 —— 备份 `[人工为主]`

目标：备份 zip 写到**隔离数据目录之外的取证区**，记 SHA256；内容含 S3/S4 状态。

### 2.1 先建取证区 `[命令]`

```powershell
# [命令] 备份取证区（必须在 $DataDir 之外）
$BackupOut = Join-Path $WorkRoot "backup_out"
New-Item -ItemType Directory -Path $BackupOut -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $RunRoot "s5") -Force | Out-Null
$BackupOut
```

### 2.2 点击流 `[人工]`

1. app 主窗口 → 顶层菜单 **`设置`** → **`备份和还原`** → 弹出标题为
   **`备份与还原`** 的窗口（若标题不是这五个字，如实记录）。
2. 在 `本地备份 / 恢复` 分区，找到 `保存到目录` 输入框，填入 §2.1 的 `$BackupOut`
   路径（从 pwsh 窗口复制完整路径粘贴进去，不要手打）。
3. 点 **`本地备份`** 按钮。
4. 看底部状态条：`处理中…` → 成功文案（形如 `本地备份完成：<路径>`）。
   失败文案（`本地备份失败…`）→ **停**，截图并保留 `$DataDir` 现场。
5. 点 **`关闭`** 关掉备份窗口（不要动 WebDAV 区任何按钮——会触网）。

### 2.3 截图 `[人工]`

- `s5/s5-backup-window-before.png` —— 填好目录、点备份**之前**。
- `s5/s5-backup-status-done.png` —— 状态条成功文案可见。
- 失败时：`s5/s5-backup-error.png`（状态条/弹窗二者都截）。

### 2.4 点击后验证 `[命令]`

```powershell
# [命令] 备份产物 + hash（只读）
Get-ChildItem -LiteralPath $BackupOut -Recurse -Force | Format-Table FullName, Length -AutoSize
$BackupZip = (Get-ChildItem -LiteralPath $BackupOut -Recurse -Filter "*.zip" -Force | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
$BackupZip
(Get-FileHash -LiteralPath $BackupZip -Algorithm SHA256).Hash.ToLowerInvariant()
# 把上一行输出抄到 s5/s5-backup-sha256.txt，连同 zip 文件名一起登记
```

- 通过判定：恰 1 个新 zip；SHA256 已记录；zip 大小 > 0。
- 内容含 S3 状态的核对是恢复阶段（S7）反向验证的；这里只记录 hash + 文件名，
  不要解压改动它（如需看清单，用只读方式另拷一份再看，原件不动）。

---

## 3. S6 —— 停止 + 重开持久性 `[人工 + 命令]`

目标：同 `V2RAYN_R_DATA_DIR` 重开后，节点/活动选择/设置与 S5 前一致。

### 3.1 S6 前快照 `[命令]`（重开前必须先取）

```powershell
# [命令]
New-Item -ItemType Directory -Path (Join-Path $RunRoot "s6") -Force | Out-Null
Get-ChildItem -LiteralPath $DataDir -Force | Format-Table Name, Length -AutoSize
(Get-FileHash -LiteralPath (Join-Path $DataDir "guiNDB.db") -Algorithm SHA256).Hash.ToLowerInvariant() | Set-Content -LiteralPath (Join-Path $RunRoot "s6\s6-db-hash-before.txt") -Encoding UTF8
Get-Content -LiteralPath (Join-Path $RunRoot "s6\s6-db-hash-before.txt")
# 有 sqlite3 时再取行清单：
sqlite3 (Join-Path $DataDir "guiNDB.db") "SELECT remarks FROM profiles ORDER BY remarks;" | Set-Content -LiteralPath (Join-Path $RunRoot "s6\s6-db-profiles-before.txt") -Encoding UTF8
```

### 3.2 停止 `[人工优先，命令兜底]`

1. `[人工]` 先试正常退出：主窗口 → **`关闭`**（或窗口右上角 × / `Alt+F4`），
   等进程真正退出（任务管理器里该 PID 消失）。
2. 若 8 秒内没退出 → `[命令]` 用 §0.4 的停 owned PID 树命令（只停 `$AppPid` 树）。
3. 记录：是正常退出还是强制停的（`exited_on_close` true/false），截图不需要，
   但要写进 observations。

### 3.3 重开 `[人工 + 命令]`

```powershell
# [命令] 同一个 $DataDir 再启动（与 §0.4 同：只设 V2RAYN_R_DATA_DIR）
$env:V2RAYN_R_DATA_DIR = $DataDir
Start-Process -FilePath $MainExe -WorkingDirectory $Pkg
$env:V2RAYN_R_DATA_DIR = $null
```

`[人工]` 等主窗口出现，核对：节点表仍是 S3 的 4 行；活动选择（如 S3 后设过）
与设置项与关掉前一致。

### 3.4 截图 `[人工]`

- `s6/s6-reopen-table.png` —— 重开后节点表（4 行可见）。

### 3.5 重开后验证 `[命令]`

```powershell
# [命令] DB hash 对比（只读；预期与 before 一致）
(Get-FileHash -LiteralPath (Join-Path $DataDir "guiNDB.db") -Algorithm SHA256).Hash.ToLowerInvariant()
Get-Content -LiteralPath (Join-Path $RunRoot "s6\s6-db-hash-before.txt")
# 有 sqlite3 时：
sqlite3 (Join-Path $DataDir "guiNDB.db") "SELECT remarks FROM profiles ORDER BY remarks;"
```

- 通过判定：hash 与 `s6-db-hash-before.txt` 一致（配置 diff 为空）。
- 不一致：把新旧清单 diff 存 `s6/s6-db-diff.txt`，记 `blocked`，保留 `$DataDir`。
- 注意：更新 `$AppPid` 为重开后的新 PID（后面 S7/清理要用）。

---

## 4. S7 —— 恢复 `[人工为主]`

目标：清空数据目录 → 从备份窗口导入 S5 备份 → 与 S5 内容一致；无残留半写。

> 破坏性步骤。S5 的 hash（`s5-backup-sha256.txt`）和 S6 的 before 快照是回退依据，
> 确认两者已落盘再往下走。

### 4.1 停 app + 清空数据目录 `[命令]`

```powershell
# [命令] 先停（同 §0.4，只停 owned 树），确认 PID 已消失再清空
Get-Process -Id $AppPid -ErrorAction SilentlyContinue
# 若仍在，用 §0.4 停止命令停掉；确认无输出后再执行下面：
New-Item -ItemType Directory -Path (Join-Path $RunRoot "s7") -Force | Out-Null
Get-ChildItem -LiteralPath $DataDir -Force | Format-Table Name, Length -AutoSize  # 清空前最后一次清单，留底
Remove-Item -LiteralPath (Join-Path $DataDir "*") -Recurse -Force
(Get-ChildItem -LiteralPath $DataDir -Force | Measure-Object).Count  # 预期 0
```

### 4.2 重启空库 `[命令 + 人工]`

```powershell
# [命令] 空数据目录启动
$env:V2RAYN_R_DATA_DIR = $DataDir
Start-Process -FilePath $MainExe -WorkingDirectory $Pkg
$env:V2RAYN_R_DATA_DIR = $null
```

`[人工]` 确认主窗口节点表为空（或首跑状态）。

### 4.3 点击流 `[人工]`

1. 主窗口 → **`设置`** → **`备份和还原`** → **`备份与还原`** 窗口。
2. 在 `本地恢复` 那一行点 **`选择备份 ZIP 恢复`** → 文件选择器里选中 S5 的
   `$BackupZip`（从 §2.4 把完整路径复制出来，在对话框地址栏粘贴定位，不要手打）。
3. 等底部状态条出现成功文案（形如 `本地恢复完成…已重载…`）。
   失败文案（`本地恢复失败（已保留现有配置）…`）→ **停**，保留现场 `$DataDir`，
   记 `blocked`。
4. 点 **`关闭`**；看主窗口节点表是否回到 4 行（S3 状态）。

### 4.4 截图 `[人工]`

- `s7/s7-restore-status-done.png` —— 状态条成功文案。
- `s7/s7-table-after-restore.png` —— 恢复后节点表 4 行。
- 失败时：`s7/s7-restore-error.png` + 保留 `$DataDir` 不删。

### 4.5 点击后验证 `[命令]`

```powershell
# [命令] 与 S5/S6 快照对比（只读）
(Get-FileHash -LiteralPath (Join-Path $DataDir "guiNDB.db") -Algorithm SHA256).Hash.ToLowerInvariant()
Get-Content -LiteralPath (Join-Path $RunRoot "s6\s6-db-hash-before.txt")
# 有 sqlite3 时：
sqlite3 (Join-Path $DataDir "guiNDB.db") "SELECT remarks FROM profiles ORDER BY remarks;"
Get-Content -LiteralPath (Join-Path $RunRoot "s6\s6-db-profiles-before.txt")
Get-ChildItem -LiteralPath $DataDir -Force | Format-Table Name, Length -AutoSize
```

- 通过判定：恢复后 DB hash / 行清单与 S6-before（= S5 备份内容）一致；
  数据目录无多余半写文件。
- journal/残留检查：把 `Get-ChildItem` 输出与 S6 前快照逐项对，发现多出来的
  `*-journal`/`*.tmp`/`*.bak` 类文件即记为可疑并存档（文件名各版本可能不同，
  所见即所得记录，不要编造固定文件名）。

---

## 5. 回滚与清理 `[命令]`

只有**全部证据已存进 `$RunRoot`**（截图 + hash txt + observations 草稿）后才清理：

```powershell
# [命令] 1) 停 owned 树（只停本 runbook 最后一个 $AppPid 树）
Get-Process -Id $AppPid -ErrorAction SilentlyContinue
# 若仍在，用 §0.4 停止命令停掉
# [命令] 2) 复核证据已落盘
Get-ChildItem -LiteralPath $RunRoot -Recurse | Format-Table FullName -AutoSize
# [命令] 3) 删隔离区（$WorkRoot 下的 extract + data_manual + backup_out 全清）
Remove-Item -LiteralPath $WorkRoot -Recurse -Force
# $RunRoot 在仓库 docs 下，不受影响，不要删
```

- 禁止动作：按名批杀进程；删仓库内其他目录；碰用户真实数据目录
  （全程只用 `$WorkRoot`/`$DataDir`，若发现路径指到用户目录立刻停）。

---

## 6. 不能精确指定的事项（诚实记录）

1. **S3 没有“从文件导入”菜单项**：README 写“从文件导入 fixture”，但
   `apps/desktop/lib/app/menu/main_menu.dart`（`_serverEntries`）里分享链接导入
   只有 `从剪贴板导入分享链接`（`ACT-MAIN-016`），文件侧只有路由规则的
   `从文件导入`（`routing_windows.dart`，与节点导入无关）。本 runbook 按源码
   实际采用“记事本复制 fixture → 剪贴板导入”路径。若实测中出现文件选择器入口，
   以实际 UI 为准并更新本节。
2. **DB 文件名 `guiNDB.db` vs `guiNNDB.db`**：`r4_32_real_entry.ps1` 用前者，
   `smoke_windows.ps1` 轮询用后者。本 runbook 以前者为准，命令里同时探测两者，
   如实记录所见。若实测落盘的是别的名字，记实际值，不要改名凑数。
3. **SQLite 行级核对依赖外部 `sqlite3`**：仓库 `tools/acceptance/` 内没有现成的
   SQLite 查询脚本（只有 `sp30_package_identity.ps1` 的 fixture 形状校验和
   `sp30_prepare_datadir.ps1` 的目录准备）。本 runbook 的 `sqlite3 …` 命令是
   可选的——无 sqlite3 的机器上跳过 DB 行查询，仅以 UI 表格截图 + 文件 hash 为据，
   并记为“DB 行级未验证”。
4. **journal/半写残留文件名**：`guiNDB.db` 的 SQLite journal/WAL 后缀名取决于
   实际 journal mode（`-journal`/`-wal`/`-shm`），仓库证据未固定。本 runbook 只要求
   “重开/恢复前后目录清单逐项对比”，不预设固定文件名。
5. **备份 zip 内布局**：`backup_controller.dart` 的成功文案只给 `root`/路径，
   未固定 zip 内部条目表。本 runbook 只断言“zip 生成 + SHA256 记录 + S7 反向一致”，
   不写死 zip 内文件清单。
6. **S4（设为活动/应用）不在本 runbook 内**：S3 后如需设活动，右键节点 →
   `设为活动`（`context_menu.dart: menuSetDefaultServer`），但“应用到回环端口”
   属于 S4（已有 armed 证据包覆盖），人工 run 里**不要**点 `重启服务` 或任何
   启动内核的入口，避免端口/OS 副作用。

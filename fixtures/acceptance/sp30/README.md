# fixtures/acceptance/sp30 — SP-30 首次使用→备份重开合成数据包

状态：准备（fixtures 已建；真实 GUI 验收未运行）。

本目录只含**合成**数据：RFC 5737 文档网段（192.0.2.0/24）、全零测试 UUID、
`sp30-synthetic-pass` 占位密码；无真实订阅地址、节点凭据、用户秘密。
端口全部 ≥11808（11881–11884），与 10808 零交集。

## 文件

| 路径 | 内容 |
|---|---|
| `synthetic-sub.txt` | 4 行合成分享链接：3×vmess（tcp/ws/tcp）+ 1×ss；remarks 为 `sp30-synth-*`，供正式 UI「从剪贴板/文件导入」使用 |

## 生成方式（确定性，可复现）

见 `tools/acceptance/sp30_package_identity.ps1 -VerifyFixture`：
对本文件逐行做形状校验（scheme 白名单 + vmess base64 可解码 + 地址落在
192.0.2.0/24 + 端口 ≥11808），不触网、不写库。

## 用途

- SP-30 步骤 S3（导入合成订阅）：正式 UI 订阅/分组入口导入本文件，预期 4 节点，
  remarks 去重后为 `sp30-synth-01/02/03/ss`。
- 夹具目录在验收中**只读**；导入一律写入隔离 `V2RAYN_R_DATA_DIR`。

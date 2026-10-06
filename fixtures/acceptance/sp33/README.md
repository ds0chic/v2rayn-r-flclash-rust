# fixtures/acceptance/sp33（合成数据，仅验收导入用）

- `synthetic-sub.txt`：4 行合成节点（3×vmess + 1×ss），remarks=`sp33-synth-*`。
- 地址全部 `192.0.2.0/24`（TEST-NET-1 文档段），端口全部 ≥11808；无真实订阅、
  无凭据、无 cookie。校验：`sp33_launch_prep.ps1 -VerifyFixture`。
- 各平台实例共用同一文件导入，导入后按实例隔离数据目录存放，不交叉借证据。

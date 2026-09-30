# fixtures/source — 原版样本与参考结果（T00 冻结）

- 来源：v2rayN 7.25.4 冻结源码（commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`），仅复制公开仓库内的模板与资源文件。
- 不含任何用户数据、订阅地址、凭据或本机路径；未运行上游程序。
- 上游项目为 GPL-3.0；复制内容保留原始版权与许可归属（见上游 `LICENSE`）。

## 目录

| 路径 | 内容 | 用途 |
|---|---|---|
| `upstream/sample/` | `v2rayN/ServiceLib/Sample/` 全量 28 个模板文件（Xray/sing-box 样例配置、TUN、DNS、路由、Mixin、PAC、证书包、平台脚本模板） | 差分验证输入、生成器对照样本 |
| `upstream/resx/` | `ResUI.resx`、`ResUI.zh-Hans.resx`、`ResUI.zh-Hant.resx` | 界面文案与字段显示名参考键 |
| `SHA256SUMS` | 上述全部文件的 SHA256 | 夹具完整性校验 |

## 使用规则

1. 差分测试将本目录样本作为“原版输入”；结果参考须由真实内核校验产生，不得手工编造。
2. 大文件（`pac`、`chrome_roots_pem`、`mozilla_roots_pem`）为原版内嵌资源，直接保留。
3. `ServiceLib.Tests/` 中的既有测试（14 个文件主题已登记在 `compat/features.yaml:existing_tests`）仍留在冻结源码树中，作为行为参考；如需进入本目录须按 T06b/T07/T08 任务卡显式复制并登记。

## 校验

```powershell
Get-Content fixtures/source/SHA256SUMS | ForEach-Object {
  $h, $p = $_ -split '  ', 2
  $calc = (Get-FileHash (Join-Path 'fixtures/source/upstream' $p) -Algorithm SHA256).Hash.ToLower()
  if ($calc -ne $h) { Write-Error "mismatch: $p" }
}
```

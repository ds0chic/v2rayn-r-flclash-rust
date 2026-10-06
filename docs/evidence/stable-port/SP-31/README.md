# SP-31 性能测量准备（基线 92d46dd）

状态：identified（测量方法 + 微基准基线完成；release GUI 采样未跑）。

本次范围：只写 `benchmarks/**`、`tools/perf/**`、本卡证据；合成场景；未改生产 Dart/Rust/C++、FRB、Cargo 锁；未 commit；
未动宿主代理/路由/TUN/DNS/Run-key；未做 release 构建（与其它代理重建互斥，本卡不重建）。
实际结果：采样方法文档 + 可复现合成基准 + 首批基线数字（见下）。
未运行范围：release 构建；release GUI 帧/启动/内存采样；`cargo` 门禁（本卡无 Rust 改动）。

## 新建文件

- `tools/perf/synth_node_bench.dart`（合成 10k 节点 filter/sort/logbatch 微基准，nearest-rank p50/p95/p99）
- `tools/perf/renderer_probe.ps1`、`tools/perf/engine_strings.py`（SP-26 探针，SP-31 复用 GPU 环境部分）
- `benchmarks/sp31_method.md`（两层采样方法：微基准已跑 / release GUI 待整合候选）
- `benchmarks/sp31_baseline_2026-10-06T17-50-29Z.json` + `..._raw.csv`（原始样本）
- 本目录 `README.md` + `observations.json`

## 命令与 exit

- `dart run tools/perf/synth_node_bench.dart --out benchmarks` → exit 0，基线见方法文档 §A。
- `flutter analyze`（apps/desktop 定向检查）→ **exit 非零，10 issues**：9 errors 全在 `test/sp21_async_page_test.dart`
 （未定义 `AsyncProfilePager/ProfilePageResult/ProfilePageRequest`）+ 1 warning（`routing_windows.dart:1603` unnecessary_cast）。
  均为并行他卡在途文件/改动（本卡在 `apps/desktop` 零新增零修改，`git status` 可证），本卡如实记录，不代修、不标绿。

## 性能基线数字（微基准，30 样本/场景）

filter_10k p50 0.73 / p95 1.48 / p99 3.65 ms；
sort_10k p50 4.43 / p95 5.73 / p99 12.33 ms；
logbatch_1k p50 0.20 / p95 0.49 / p99 1.06 ms。
门槛对照：10k 搜索排序 p95 ≪ 200ms；feedback≤100ms 在数据层无压力，待 GUI 层验证帧预算与全量重绘行为。

## 下一步前置（需主控派单）

release GUI 采样由整合候选执行（单 release 构建 + 合成实 DB + DevTools Timeline，cold 20 / warm 30），owner 为整合者；
本卡脚本与阈值开箱即用。`git status --short` 见 observations.json。

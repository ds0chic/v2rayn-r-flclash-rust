# SP-31 可复现采样方法（合成场景，release GUI + 微基准两层）

阈值来源：`docs/repair/stable-port-2026-10-06/ACCEPTANCE_MATRIX.md §4`
（feedback p95≤100ms；1k 冷≤2s/热≤1s；10k 搜索排序 p95≤200ms；
UI/raster 各 p95≤16.7ms 且超预算≤1%，同时记总帧时间；内存/线程/队列记基线+无持续增长）。

## A. 微基准层（本卡已跑，纯合成，无 GUI）

- 工具：`tools/perf/synth_node_bench.dart`（pure Dart，无 Flutter/生产导入）。
  `dart run tools/perf/synth_node_bench.dart --out benchmarks/`。
- 固定量：seed=20261006，10k 合成节点（remark/protocol/group/latency 合成字段），
  每场景 30 样本；percentile 统一 nearest-rank：`rank=ceil(p/100*n)` 取排序后第 rank 个。
- 场景：`filter_10k_ms`（substring 1 查询）、`sort_10k_ms`（remark 全排序）、
  `logbatch_1k_ms`（1000 行连接日志组装）。
- 输出：`benchmarks/sp31_baseline_<UTC>Z.json`（p50/p95/p99/min/max）+
  `benchmarks/sp31_baseline_<UTC>Z_raw.csv`（原始样本，审计用）。
- 本次基线（基线 commit 92d46dd，`sp31_baseline_2026-10-06T17-50-29Z.json`）：
  filter_10k p50=0.73ms p95=1.48ms p99=3.65ms；
  sort_10k p50=4.43ms p95=5.73ms p99=12.33ms；
  logbatch_1k p50=0.20ms p95=0.49ms p99=1.06ms。
  结论：纯数据层 10k 搜索/排序远低于 200ms 门槛一个数量级以上；GUI 侧瓶颈应在构建/绘制/全量重绘，不在过滤排序本身。

## B. release GUI 层（方法已定，本卡未跑，由整合候选执行）

- 前置：固定 commit 的 `flutter build windows --release` 单候选（不与其它代理并行重建），armed=false，合成实 DB（1k/10k/100k 节点，`fixtures/` 合成生成器），端口≥11808，宿主代理/路由/TUN/DNS/Run-key 零写入。
- 采样：DevTools Timeline / `flutter run --profile --trace-startup --trace-skia`：
  冷启动 20 样本 / 热启动 30 样本（ACCEPTANCE_MATRIX §4 数量）；列表滚动取 UI/raster 帧时间；
  筛选输入→首帧反馈计时；日志连接更新取 burst 下帧超预算比例；内存/线程/队列记基线快照。
- 记录：原始 timeline + p50/p95/p99 + 超预算帧占比 + 总帧时间；cold/warm 分开；同一硬件/核/配置。
- 未运行原因：需 release 候选 + 真机交互，不在本卡"测量准备"范围内；方法与脚本就绪后由 SP-31 接线/整合任务执行。

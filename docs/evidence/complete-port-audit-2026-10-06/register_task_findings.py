"""Append current audit pointers without replacing prior implementation records."""
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
TASKS = ROOT / "docs/repair/tasks"
findings = {
    "R4-01": "CP-01/03：真实核心退出仍Running；applied target并发读取错误；apply job终态未结束。",
    "R4-04": "CP-02/03：启动A→停止→启动B执行A/B/stop；超时结果未知未对账；目标/operation/job身份需冻结。",
    "R4-05": "CP-04：TUN清理失败仍删journal/归属并报成功；退出不能隐藏未确认资源。",
    "R4-06": "UI-05/06/10：800px布局需横滚，运行节点身份/测试入口/最新消息呈现未齐；控件滚动后可达。",
    "R4-09": "CP-15：真实FRB/SQLite10k同步读取102–123ms；100k同分页Dart循环约2982ms；仅绘制虚拟化不足。",
    "R4-10": "CP-10/15：All导入逐条同步写；数据读取长路径仍同步，需真实交互不阻塞证据。",
    "R4-12": "CP-05/09/14：独立窗口旧revision重试、丢保存结果永久等待、主题字体语言未继承。",
    "R4-14": "CP-08/09：畸形/失败读取变可写空快照；部分删除后旧全稿复活a；丢回包无有界处理。",
    "R4-16": "CP-10：All/无组第二条失败留下首条，旧preview可材料化Custom文件，新preview/commit未接正式入口。",
    "R4-17": "UI-04/07：当前组没canonical持久化/恢复，新增编辑入口没正确当前对象；已有更新来源修复保留。",
    "R4-19": "CP-SET-08：Mihomo merge/custom生产计划仍缺消费者，不能用merge helper单测关闭正式运行链。",
    "R4-23": "CP-01/03、UI-09：实际退出/运行节点事实与job终态，连接表列持久化/右键/排序与虚拟化缺口。",
    "R4-24": "CP-SET-09：平台去重key忽略内容变化，模式保存失败被忽略，自定义PAC缺文件语义不齐；OS本轮未写。",
    "R4-25": "CP-04/12：清理失败保留/回传、24h helper保活、实际lease标签、主/sidecar提权探活退出和IPv6保护尚未完成。",
    "R4-27": "CP-06/07/08：坏字段整树默认、空配置伪成功、canonical IndexId往返和失败读取快照不可提交。",
    "R4-29": "CP-13/16：当次flags没接UI，自有源/可信公钥接线缺失；验签算法已实现，不能重复称Unsupported。",
    "R4-31": "CP-15/16：真实100k已读回但同步约3s；完整releaseGUI帧/资源/24h与500切换仍未验，测试进程异常待查。",
    "R4-32": "CP-16：最新Windowsrelease构建通过但原测试门禁未全绿；dist旧672e666+dirty，Dart AOT/后端文件与新build不同。",
    "R4-33": "其它五OS×架构完整发行/运行仍未验证；不能以Windows编译带过平台实例。",
    "R4-34": "CP-SET-08：外部路由模板下载/事务入口、本地SRS生产来源选择仍缺链；资源下载存在不能证明最终消费者。",
}
marker = "## 2026-10-06 完整稳定复审"
for task, note in findings.items():
    path = TASKS / f"{task}.md"
    text = path.read_text(encoding="utf-8")
    if marker in text:
        continue
    text += f"\n\n{marker}\n\n状态：identified（本轮发现与未完成合同；不覆盖历史实现记录）。基线a7aa0a5。\n\n{note}\n\n详见 [总审计与验收方案](../../evidence/complete-port-audit-2026-10-06/README.md) 及其领域证据。生产源码本轮未修改；实际命令结果/故障证据按总报告，未实际OS操作仍未验证。当前不能提升为完整稳定版verified。\n"
    path.write_text(text, encoding="utf-8")
start = ROOT / "docs/repair/START_HERE.md"
text = start.read_text(encoding="utf-8")
intro = "## 2026-10-06 最新复审入口\n\n实施前先读 [当前全范围审计与稳定验收方案](../evidence/complete-port-audit-2026-10-06/README.md)，基线a7aa0a5。它复核并关闭部分旧缺陷，同时新增真实核心退出仍Running、命令次序、applied身份、清理假成功、备份活动节点往返、坏配置默认化、All导入半写、独立窗口回包/重试等阻断。旧计划与任务历史保留；不能用旧implemented/测试绿或旧ZIP宣布当前完成。先把正确期望回归转绿，再从当前未武装包普通入口逐项验收。\n\n"
if "## 2026-10-06 最新复审入口" not in text:
    pos = text.find("\n") + 1
    start.write_text(text[:pos] + "\n" + intro + text[pos:], encoding="utf-8")
print(f"Registered audit pointers for {len(findings)} task cards; historical records retained.")

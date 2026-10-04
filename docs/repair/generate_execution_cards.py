"""Generate documentation only; product code and compat ledgers are untouched."""
import collections
import csv
import json
from pathlib import Path
import yaml

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent
UP = "work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/"
BASE = "77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5"
FROZEN = "7d6a967c18c697f28dc6917122ed3a4993fcf336"
# id, title, unique flow, dependencies, modules, upstream symbols, acceptance
SPECS = [
("00","冻结证据与完整库存","对同一合成数据执行一条原版/当前用户流程并登记差异","无","docs/repair、docs/evidence、tools/audit；compat仅追加","Views/ViewModels/ConfigHandler/TaskManager；各台账source_file/source_symbol","锁定两个commit、包hash、armed=false；所有库存分配owner；正常/错误/取消/重开/效果场景齐全；未证实不提升verified"),
("01","启动重试与真实运行事实","启动失败后修好原因，在同一GUI显式重试","00","runtime_controller/bridge、api/engine、application/engine、运行DTO","MainWindowViewModel.Reload/LoadCore；ConfigHandler.SetDefaultServer","根三条失败契约转绿；历史snapshot.error不否决新命令；错误不转空DTO；applied目标/revision/hash可信；sameactive no-op不报告运行成功"),
("02","显式启动目标与默认合同","选中B后顶部启动B，F5仍重载默认节点","01；27存储失败合同","profile_actions/controller、main_shell、application/set_active、runtime目标入口","ProfilesViewModel.SetDefaultServer；MainWindowViewModel.Reload；ConfigHandler.SetDefaultServer","无active/Aactive+Bselected/多选primaryB/无目标；点击时冻结ID；普通单击不启用；ID变化bump revision且persist失败不留内存伪active；默认恢复按原版"),
("03","缺核恢复与配置校验有界","缺核/坏配置后安装或修复并再次启动","01、02","core locator、core_adapters、net_host/session、缺核UI接入","CoreInfoManager/CoreHandler；MainWindowViewModel.LoadCore；CheckUpdateViewModel","正式下载目录定位；不靠开发XRAY_BIN；坏核/空格路径/端口冲突可读；预检Command.output有deadline且终止受管子进程；修好可重试"),
("04","命令队列与迟到响应","连续启动停止重试仍反馈及时并落到可信最终状态","01","runtime_controller、net_host_client、IPC、net_host命令调度","Reload的_hasNextReloadJob；原版异步反馈；运行状态机","本地pending立即可见且不假造Running；队列有界；deadline包括排队；refresh合并；旧session响应淘汰；超时后无累积worker；取消结果未知先reconcile"),
("05","统一退出与快速重开","运行中真退出后立刻重开或交接自更新","01、04","desktop lifecycle、updater handoff、host lease、managed runtime、monitor flush","MainWindowViewModel.CloseApp；托盘退出；ProcUtils；TaskManager","隐藏不停止；真退出有界stop/drain/flush/平台恢复；host重连返回actual目标/core/端点，不猜desired/Xray；更新runner没有旧核文件锁；失联兜底不是正常退出"),
("06","底栏可操作分区","用户辨认底栏代理/TUN/路由控件并读取运行与速度","00","status_bar_view、桌面视觉token、组件测试","WPF StatusBarView.xaml/StatusBarViewModel","两行入站/速率与原版顺序；下拉边框箭头焦点；技术诊断移详情；800/1200/1440宽、长文本、100–200%DPI可达；不缩字/不整栏横滚"),
("07","右键生命周期与目标快照","选节点打开右键及子菜单，执行或取消后继续操作","08","profiles_table/profile_actions菜单、context_menu_session","ProfilesView.xaml/.cs DataGrid ContextMenu与鼠标键盘事件","行/表头/空白、多选primary、外点/Esc/失焦/滚动/右键换行；只有一个菜单session；边缘DPI定位；捕获目标不漂移，失效拒绝；恢复焦点"),
("08","即时选择与键盘对象","单击或Ctrl/Shift点击后50ms内松键并Enter/启动","00、02目标合同","profiles_table/controller选择焦点、profiles_page","ProfilesView.xaml.cs；ProfilesViewModel.RefreshServersBiz；本地Flutter Gesture SDK","两条短时序失败用例转绿；选中不等300ms；modifier在事件时捕获；不双toggle；双击按设置编辑/激活；拖选/导航/主行/分组重开同原版"),
("09","真实数据分页与增量","打开大量节点并在测速时切组筛选排序","08、10异步桥合同","BridgePort摘要分页、profile controller、store_repo、delta DTO","RefreshServers/GetProfileItemsEx；ProfileExItem.Sort；原版列合同","真实DB10k/100k/100k+无游标截断；不两次全量读；草稿详情按ID；结果统计增量；页边界选择范围正确；旧搜索不盖新结果"),
("10","异步桥与慢I/O","备份/磁盘/数据库慢时仍可滚动窗口和查看反馈","04","手工FRB API、BridgePort/controller异步、受限worker；生成统一整合","原版异步业务入口；原方案§04/05线程边界","I/O/DB批处理/哈希/IPC移出UI；sync只保留测量极短内存方法；monitor磁盘flush移出共享读锁；慢任务中帧/取消可用；FRB二次生成no-diff"),
("11","字段表单和默认值","修改一个原版字段后分别保存、取消、重开","00","profile/settings/DNS/routing字段schema和草稿UI","冻结Views绑定、Models/Configs/Enums、ConfigHandler初始化","逐字段组核对180设置/158实体适用UI；null/缺省/空串/联动/范围/校验/Tab顺序；不以JSON编辑器替代原版可见功能"),
("12","窗口和三布局","从主窗进入原版子窗完成操作再回正确焦点","00","main_shell、Windows runner/host、子engine消息、dialog布局","MainWindow/WindowBase及53窗口库存；Owner/modal/关闭结果","三布局与53窗口逐条；现有独立窗核对Owner/modal/几何/revision/DPI/主题；读失败不提交空草稿；原版子提交/即时提交不改全窗确定取消"),
("13","设置保存到实际效果","在一个设置分组改字段并按原版时机生效","10、11、12","settings actions/controller、application消费者、codegen/platform映射","OptionSettingViewModel.SaveSetting；MainWindowViewModel返回/Reload；字段apply_timing","逐分组实例保存→DB/JSON→重开→真实生成/平台效果；错误回传不无条件成功；stale草稿拒绝；需要重启提示；180字段无仅保存漏线"),
("14","路由原版提交与导入","确认子编辑/修改策略后关闭路由主窗并验证规则生效","10、12","routing actions/controller/windows/host、application routing、codegen","RoutingSettingViewModel策略即时保存/子编辑提交；关闭IsModified；ConfigHandler.InitRouting","恢复原版每个提交点；每次提交事务化且检查结果；内置导入真执行；多选/全选/Enter/Delete；关闭Modified触发Reload；规则顺序和请求路径正确"),
("15","DNS草稿保护与Reload","将DNS改为另一字段原值，应用预设后保存生效","10、11","dns_window/controller、入口actions、application/dns、codegen/dns","DNSSettingViewModel.SaveSetting；MainWindowViewModel DNS保存成功Reload","碰撞故障用例转绿；baseline用fieldId不以text；应用/确定/取消原版一致；预设保留改动；假IP/hosts/IPv6/TLS核差异；下载失败保留草稿"),
("16","导入导出单次提交","当前组粘贴扫码或文件导入再导出由原版再导入","17来源合同、27","import/export UI、api/subs、subscriptions解析、application批提交","AddBatchServersCommon；MainWindowViewModel剪贴板扫码；Export2Share/Inner/ClientConfig","17格式逐条；parse/preview与commit分离；无Rust+Dart重复写；原版来源/去重/组快照；取消/坏行/文件不可写；字段原版互通和重开"),
("17","订阅保护与稳定身份","更新当前A组后保留应保留节点、默认和统计并继续启动","00、27事务合同","subs UI/API、application/subs/engine/store_repo、merge/统计映射","SubscriptionHandler.UpdateProcess；RemoveServersViaSubid/FindMatchedProfileItem/CloneServerStatItem","P0先修：IsSub=false保留，更新只替换IsSub=1；new/clone按原版逐入口不统一false；稳定ID/默认/统计；A页不更新B；All→合法全部；SubIndexId重开；取消失败不损数据"),
("18","协议节点完整闭环","新建一种协议节点到握手、编辑、重开全过程","02、11、16、27","专用editor/fields、domain能力、application codegen、core_adapters","AddServerViewModel/AddServerCommon；ProfileItem/ProtocolExtra/TransportExtra；各CoreConfigGenerator","11基础协议逐实例；所有适用TLS/Reality/传输字段；保存/重开/生成/合成服务握手；错误定位字段；核不支持不能静默掉字段；真实数据不经FRB"),
("19","自定义模板Mixin","自定义配置或出站结合模板/Mixin后启动重开","03、13、14、15","custom/template/mixin UI及application/codegen","AddServer2/FullConfigTemplate；PreSocks/Custom/Outbound生成合同","原文/来源/核/日志/PreSocks/实际统计端点；模板优先级/覆盖/tag冲突；真实校验和本地请求；缺文件恢复；取消不写；不强制转Mihomo YAML"),
("20","策略组和代理链","引用多个节点形成组或链后验证真实请求路径","18、19","group editor/picker、application groups、domain、codegen拓扑","AddGroupServer/ProfilesSelect；CoreConfig组和链生成","组模式/区域生成/顺序/嵌套/循环/成员删除/跨订阅ID变化；真实链顺序和策略切换；port0默认规则按原版；重开无悬空引用；取消不写"),
("21","十四内核普通入口矩阵","从普通UI安装一个核并请求本地合成服务","03、18–20、13–15","update/core locator、adapters/runtime/host、codegen核版本能力","CoreInfoManager各CLI/LockedMaxVersion/CoreUrls；CoreHandler","14核逐版本/校验/启动/握手/流量/停止/失败重试；适用协议和平台；正式目录不靠dev env；锁版本与许可；脚本version/minimal session不能替代UI链"),
("22","测速范围取消和代际","执行一种测速、取消再开始并排序本轮结果","04、09、18、21","speed UI/controller、api/speedtest、application runner/result_store","SpeedtestHandler/ProfilesViewModel各动作范围；ProfileEx结果排序","TCP/真实延迟/速度/UDP/IP逐适用动作；viaProxy/TLS；Stop确认结束；旧job不串；失败不是0ms；结果增量不全库poll；临时核清理"),
("23","监控连接日志统计","运行中查看监控并切会话或隐藏后再查看","04、09、10、21","monitor UI/controller/API、application监控、统计存储","MsgViewModel；ClashConnections/Proxies；StatisticsHandler/ServerStatItem","采集暂停与滚动暂停分开；连接/组操作是真API；迟到响应按session淘汰；锁外flush；归属actual目标；隐藏页不停止采集；统计重开"),
("24","代理PAC与applied端点","选代理模式后改入站端口并退出恢复","05、13、21","platform controller/service、runtime成功协调、PAC、host恢复","LoadCore→UpdateSysProxy；四模式/PAC/退出恢复","授权隔离机四模式、每次apply后实际端点协调、PAC服务、逐应用即时生效、所有权恢复；persist与外部apply分别报告；不写当前宿主"),
("25","生产TUN和拒绝恢复","隔离机开启真实auto-route TUN再关闭或故障恢复","05、15、21、24","TUN DTO/UI/usecase、tun_plan、helper、host租约","TunModeItem；原版权限/TUN/路由/DNS","脱敏实际lease贯穿FRB；开启/关闭/取消检查真实结果；失败保旧不说已关闭；授权隔离VM生产路由/DNS/IPv6/排除/崩溃睡眠恢复，未具备则blocked"),
("26","托盘热键自启动作","从托盘或OS热键执行原版动作并隐藏重开","02、05、12、13","tray model、desktop integration、hotkeys/插件、autostart回传","StatusBar/Tray；GlobalHotkeySetting；AutoRun/AutoHideStartup","15托盘/9热键逐动作；复制代理命令真实剪贴板；图标含义原版核对；同组合/冲突/暂停派发门/注销；自启失败反馈；有副作用OS项隔离"),
("27","存储迁移和失败恢复","打开或迁移失败时明确恢复，保存重开一致","00","engine初始化、application/persistence、UiStateStore路径与写入","LoadConfig/升级/数据目录；实体与窗口状态","production不能fallback内存/NullRuntime假Accepted；坏DB/只读/磁盘满/锁冲突可见；158字段/历史/未知值；候选失败保原件；UI状态写数据目录异步原子，兼容旧路径"),
("28","备份WebDAV与恢复","备份到本地或合成远端，恢复重开并验证状态","05、10、17、27","backup UI/controller/API、backup lifecycle/WebDAV","BackupAndRestoreViewModel/Handler；Replace/Merge区别","本地ZIP/WebDAV正常及401/404/断连/超时；busy finally；取消/重复恢复/任务drain；失败保原数据；窗口主题/组/节点/热键/runtime重载；工作目录按operation"),
("29","本项目发行与升级","检查真实本项目包验签后退出交接升级或回滚","03、05、27","update controller/API/service、updater runner、release信任配置","CheckUpdate/UpdateService；核App分流和本项目发布协议","release repo不再恒None须真实产品前置；自有信任根/资产协议；拒上游C#包；验签/异常busy/断网/取消/回滚；清理后交接；无真实repo公钥则blocked"),
("30","完整语言和错误反馈","切原版语言后主子窗口和所有错误均可读重开","06、11、12","locale/产品资源/全UI消费、code→message、通知协调","冻结ResUI语言资源与RTL/快捷键/错误文案","九语言产品文字资源而非仅Material locale；长文本/RTL；子engine主题字号locale一致；新消息不被旧平台提示遮住；故障原因+行动+脱敏详情"),
("31","真实负载性能稳定","代理测速日志并发时持续操作并测资源增长","05、06、08–10、22、23","trace/benchmarks、热点最小优化、性能证据","同机原版基准；原方案性能目标；总方案§8","未武装release真实DB10k/100k；p95/p99反馈/帧/搜索/首帧/IPC队列分别测；24h负载与500次切换；不换Synthetic Bridge隐藏慢路；未达不能说最佳"),
("32","Windows未武装包交付","干净机解压或安装最终包走全部Windows适用流程","所有Windows适用卡通过；不等待33才发Windows阶段包","release/installer、README/NOTICE/build-info、证据库存追加","冻结WPF全适用清单及T20交付要求","全部Rust/Flutter门禁、FRBno-diff、armed=false；依赖闭包/升级卸载；UI首次启动核下载真实请求；同commit/hash；P0/P1零；不AUTO_SMOKE绕按钮/预置active"),
("33","六平台完整移植","在一个目标OS×架构安装并按当地原版执行适用流程","32形成Windows阶段合同；逐平台实例执行","对应runner/plugin/platform、包/CI/核与平台证据","Windows WPF；macOS/Linux Avalonia；platform-matrix","六单元GUI实际构建安装运行/核/代理TUN权限/窗口/热键/流量；不是cargo check；所有适用条目verified；额外架构差异保留，无硬件SDK则blocked"),
("34","资源源和后台任务","保存资源源或全局周期后实际下载自动更新重开","14、15、17、29资源客户端合同","资源服务/调度、下载API、Geo/SRS/模板/转换UI消费者","TaskManager.SetupTaskHandler；UpdateGeoFileAll/CheckHasUpdate；Global/SubscriptionHandler转换","Geo/SRS/路由DNS模板/SubConvert/区域/证书源逐消费者；内置真导入；全局AutoUpdateInterval周期与更新检查；虚拟时钟+合成下载+真实重开；仅存URL不算"),
]
FEATURE_OWNER = {"profile":"18","subscription":"17","import-export":"16","test":"22","routing":"14","dns":"15","core":"21","system-proxy":"24","tun":"25","monitor":"23","desktop":"26","app-mgmt":"29","backup":"28","help":"30"}
ACTION_OWNER = {"subscriptions":"17","settings":"13","routing":"14","dns":"15","hotkey":"26","backup":"28","profiles":"08","testing":"22","tray":"26","statusbar":"06"}
MAIN_OWNER = {**{n:"18" for n in range(1,12)},12:"19",13:"19",14:"20",15:"20",16:"16",17:"16",18:"16",19:"17",20:"17",21:"17",22:"17",23:"17",24:"13",25:"14",26:"15",27:"19",28:"26",29:"26",30:"23",31:"28",32:"34",33:"34",34:"34",35:"01"}
WIN_OWNER = {1:"26",2:"26",3:"30",4:"26",5:"29",6:"29",7:"28",8:"30",9:"05",10:"26",11:"26",12:"12"}
MODULE_PATHS = {
 "00":["docs/repair","docs/evidence","compat"],
 "01":["apps/desktop/lib/features/runtime","crates/bridge_api/src/api/engine.rs","crates/application/src/engine.rs","crates/ipc_contract"],
 "02":["apps/desktop/lib/features/profiles/profile_actions.dart","apps/desktop/lib/features/profiles/profiles_controller.dart","apps/desktop/lib/app/shell/main_shell.dart","crates/application/src/engine.rs"],
 "03":["crates/core_adapters","crates/runtime","services/net_host/src/session.rs","crates/application/src/engine.rs","apps/desktop/lib/features/update"],
 "04":["apps/desktop/lib/features/runtime","crates/application/src/net_host_client.rs","crates/ipc_contract","services/net_host/src"],
 "05":["apps/desktop/lib/app/shell/desktop_integration.dart","apps/desktop/lib/features/update/update_controller.dart","services/net_host/src","crates/application/src/engine.rs","crates/bridge_api/src/api/monitor.rs"],
 "06":["apps/desktop/lib/app/shell/status_bar_view.dart","apps/desktop/lib/shared/theme"],
 "07":["apps/desktop/lib/features/profiles/profiles_table.dart","apps/desktop/lib/features/profiles/profile_actions.dart","apps/desktop/lib/shared/widgets/context_menu_session.dart"],
 "08":["apps/desktop/lib/features/profiles/profiles_table.dart","apps/desktop/lib/features/profiles/profiles_controller.dart","apps/desktop/lib/features/profiles/profiles_page.dart"],
 "09":["apps/desktop/lib/bridge/bridge_port.dart","apps/desktop/lib/features/profiles","crates/application/src/store_repo.rs","crates/bridge_api/src/api/engine.rs"],
 "10":["crates/bridge_api/src/api","apps/desktop/lib/bridge/bridge_port.dart","apps/desktop/lib/features","crates/application/src","crates/bridge_api/src/api/monitor.rs"],
 "11":["apps/desktop/lib/features/profiles","apps/desktop/lib/features/settings","apps/desktop/lib/features/routing"],
 "12":["apps/desktop/windows/runner","apps/desktop/lib/app/shell/main_shell.dart","apps/desktop/lib/features/settings","apps/desktop/lib/features/routing","apps/desktop/lib/shared/widgets"],
 "13":["apps/desktop/lib/features/settings","crates/application/src","crates/config_codegen","crates/bridge_api/src/api/settings.rs"],
 "14":["apps/desktop/lib/features/routing","apps/desktop/windows/runner","crates/application/src/routing.rs","crates/bridge_api/src/api/routing.rs","crates/config_codegen"],
 "15":["apps/desktop/lib/features/routing/dns_window.dart","apps/desktop/lib/features/routing/dns_controller.dart","apps/desktop/lib/features/routing/routing_actions.dart","crates/application/src/dns.rs","crates/bridge_api/src/api/dns.rs","crates/config_codegen"],
 "16":["apps/desktop/lib/features/subs","apps/desktop/lib/features/profiles/profile_actions.dart","crates/bridge_api/src/api/subs.rs","crates/subscriptions","crates/application/src/subs.rs"],
 "17":["apps/desktop/lib/features/subs","apps/desktop/lib/features/profiles/profiles_controller.dart","crates/bridge_api/src/api/subs.rs","crates/application/src/subs.rs","crates/application/src/engine.rs","crates/application/src/store_repo.rs","crates/subscriptions/src/merge.rs"],
 "18":["apps/desktop/lib/features/profiles","crates/domain","crates/application/src/codegen.rs","crates/config_codegen","crates/core_adapters"],
 "19":["apps/desktop/lib/features/profiles","crates/application/src/custom.rs","crates/application/src/mixin.rs","crates/config_codegen"],
 "20":["apps/desktop/lib/features/profiles","crates/application/src/groups.rs","crates/domain","crates/config_codegen"],
 "21":["apps/desktop/lib/features/update","crates/core_adapters","crates/runtime","services/net_host","crates/config_codegen","tools/cores"],
 "22":["apps/desktop/lib/features/profiles","crates/bridge_api/src/api/speedtest.rs","crates/application/src/speedtest.rs"],
 "23":["apps/desktop/lib/features/monitor","crates/bridge_api/src/api/monitor.rs","crates/application/src/monitor.rs"],
 "24":["apps/desktop/lib/features/settings/platform_controller.dart","crates/application/src/platform_service.rs","crates/platform","services/net_host"],
 "25":["apps/desktop/lib/features/runtime/tun_toggle.dart","apps/desktop/lib/features/runtime/runtime_bridge.dart","apps/desktop/lib/app/shell/status_bar_view.dart","crates/application/src/tun_plan.rs","crates/bridge_api/src/api/contract.rs","services/privileged_helper","services/net_host"],
 "26":["apps/desktop/lib/app/shell/tray_menu_model.dart","apps/desktop/lib/app/shell/desktop_integration.dart","apps/desktop/lib/features/settings/hotkeys.dart","crates/platform"],
 "27":["crates/bridge_api/src/api/engine.rs","crates/application/src/engine.rs","crates/persistence","apps/desktop/lib/features/profiles/ui_state_store.dart"],
 "28":["apps/desktop/lib/features/backup","crates/bridge_api/src/api/t16.rs","crates/application/src/backup_service.rs"],
 "29":["apps/desktop/lib/features/update","crates/bridge_api/src/api/t16.rs","crates/application/src/update_service.rs","crates/updater","tools/release"],
 "30":["apps/desktop/lib/app/locale_config.dart","apps/desktop/lib/features","apps/desktop/lib/app","apps/desktop/lib/shared/widgets"],
 "31":["apps/desktop/lib/perf","benchmarks","tools"],
 "32":["tools/release","README.md","NOTICE.md","docs/evidence","compat"],
 "33":["apps/desktop","crates/platform","tools/release","compat/platform-matrix.md"],
 "34":["crates/application/src","crates/bridge_api/src/api","apps/desktop/lib/features/subs","apps/desktop/lib/features/update","apps/desktop/lib/features/routing"]
}
def load(file):
    return yaml.safe_load((ROOT/"compat"/file).read_text(encoding="utf-8"))
def window_owner(window):
    for key,owner in [("StatusBar","06"),("DNS","15"),("Routing","14"),("Sub","17"),("Backup","28"),("Update","29"),("Clash","23"),("MsgView","23"),("Hotkey","26"),("Server2","19"),("Template","19"),("Group","20"),("ProfilesSelect","20"),("AddServer","18"),("ProfilesView","08")]:
        if key in window: return owner
    return "12"
rows=[]
def add(file,collection,item,owner,ordinal):
    identity=item.get("id") or item.get("name") or item.get("symbol") or item.get("window") or f"inventory:{collection}:{ordinal:03d}"
    rows.append({"ledger":file,"collection":collection,"id":str(identity),"owner_task":"R4-"+owner,"previous_status":str(item.get("status","missing")),"repair_status":"identified","name":str(item.get("name",item.get("window",item.get("symbol","")))),"source_commit":item.get("source_commit",FROZEN),"current_evidence":"","acceptance_scope":"正常/错误/取消/重开/真实效果，按平台适用性"})
for file,collection in [("features.yaml","features"),("actions.yaml","items"),("fields.settings.yaml","items"),("fields.entities.yaml","items"),("layouts.yaml","items")]:
    for ordinal,item in enumerate(load(file)[collection],1):
        if file=="features.yaml": owner=FEATURE_OWNER.get(item.get("family"),"00")
        elif file=="actions.yaml":
            owner=ACTION_OWNER.get(item.get("domain"),"00")
            if item["id"].startswith("ACT-MAIN-"):
                owner=MAIN_OWNER.get(int(item["id"].rsplit("-",1)[1]),"00")
            elif item["id"].startswith("ACT-WIN-"):
                owner=WIN_OWNER.get(int(item["id"].rsplit("-",1)[1]),"00")
        elif file=="fields.settings.yaml": owner="13"
        elif file=="fields.entities.yaml": owner="27"
        else: owner=window_owner(item.get("window",""))
        add(file,collection,item,owner,ordinal)
for file,collections_map in {
    "features.yaml":{"config_types":"18","core_capability_matrix":"21","fmt_formats":"16","scheduled_and_background":"34","existing_tests":"00","enum_scan":"00"},
    "actions.yaml":{"hotkey_scope_rules":"26","scheduled_tasks":"34"},
    "fields.entities.yaml":{"migration_paths":"27","reference_semantics":"27","storage_tables":"27"},
    "layouts.yaml":{"main_layouts":"12","window_inventory":"12"}
}.items():
    doc=load(file)
    for collection,owner in collections_map.items():
        for ordinal,item in enumerate(doc[collection],1): add(file,collection,item,owner,ordinal)
OUT.mkdir(exist_ok=True)
(OUT/"tasks").mkdir(exist_ok=True)
with (OUT/"coverage.csv").open("w",encoding="utf-8-sig",newline="") as stream:
    writer=csv.DictWriter(stream,fieldnames=list(rows[0]))
    writer.writeheader()
    writer.writerows(rows)
def render(spec,suffix="",ids=None,parameter=None):
    num,title,flow,deps,modules,symbols,accept=spec
    tid="R4-"+num+suffix
    if parameter: flow+=f"；本次唯一参数实例：{parameter}"
    mapping="、".join(ids) if ids is not None else f"coverage.csv 的owner_task=R4-{num}全部条目及总方案J矩阵的跨域关联；逐条检查，不重复计数"
    commands=f"新增本卡行为测试后在apps/desktop执行flutter analyze、flutter test test/{tid.lower().replace('-','_').replace('.','_')}_contract_test.dart（这是待实现文件名，不宣称已有）；Rust改动执行cargo fmt --all -- --check、cargo clippy --workspace --all-targets --locked -- -D warnings、cargo test --workspace --locked。原生/运行改动需要flutter build windows --release和本卡真实场景；最终R4-32全门禁。"
    if num=="00": commands="git rev-parse HEAD；清点coverage.csv和源台账集合一致，逐条检查owner与旧证据commit；完成合成原版对照采样。文档卡不宣称Rust/Flutter测试通过。"
    return f"""# {tid} {title}

状态：identified（待执行，本文件不是完成报告）。

任务 ID：{tid}

本次唯一用户流程：{flow}。

前置任务及已验证证据：{deps}。前置须达到对应卡完成条件，当前没有宣称它们已验证。本轮缺陷证据在docs/evidence/user-flow-audit-2026-10-05/；mock只证明故障分支，不证明真实运行。

对应 feature / field / action / layout ID：{mapping}。新增/未定范围由R4-00追加分配；协议/实体对应行读domain-map，不虚构ID。

必读上游文件、符号和固定 commit：原版{FROZEN}，UP={UP}；{symbols}及相关台账source_file/source_symbol。应用基线{BASE}；读现有实现/原任务卡但不复用其完成宣称。

输入、输出、错误、取消、权限、持久化及生效语义：只用公开源码/合成输入；点击时冻结目标、modifier、revision/operation；提交按原版，输出必须区分持久化与运行/平台效果。错误可读且可重试；取消只撤未提交部分，结果未知先reconcile；不假造成功/Running。测试先探测端口且≥11808，不碰10808、不改宿主代理/TUN/路由/Run-key；OS效果在授权隔离机验，前置缺失写blocked。本卡特别合同：{accept}。

允许修改的模块：{modules}；当前定位路径：{'；'.join(MODULE_PATHS[num])}。仅改本卡流程相关符号，宽目录不授权重写无关模块；对应行为测试、本卡、docs/evidence/repair/{tid}/。FRB生成由一个接口整合者操作，其他模块不得跨范围改。

禁止改变的已有行为：原版选择/范围/字段默认与null/提交时机/确认取消/平台差异；sameactive默认动作幂等、All当前订阅原版范围、IsSub逐入口规则保留；work/outputs只读；compat只追加，不降分母；数据面不经过Flutter/FRB。

测试夹具和原版预期：根据总方案J01–J18选本卡流程；两套应用相同合成夹具，从可见入口开始→真实FRB/SQLite→重开→配置/核心/平台实际效果。正常、错误、取消、并发/重开都记录预期与实际；字段/核/平台矩阵逐实例验，fake只作故障注入。

本次必须通过的命令/真实场景：{commands} 必过场景：{accept}。

证据文件位置：docs/evidence/repair/{tid}/，待创建README.md、observations.json、命令日志、真实截图/效果与清理记录；固定commit/hash、armed=false、fixture/内核/系统/DPI版本。当前没有完成证据。

完成条件：{accept}。对应库存适用项逐条有当前证据才提升verified；未运行写未运行，未实测写未验证。只保存/函数存在/旧单测绿不能完成。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。记录提供方/调用方/输入输出/错误/生效时机，由整合者固定接口和唯一写入者；继续独立工作，不绕过正式入口。
"""
for spec in SPECS:
    (OUT/"tasks"/f"R4-{spec[0]}.md").write_text(render(spec),encoding="utf-8")
instances=[]
groups=collections.defaultdict(list)
for field in load("fields.settings.yaml")["items"]:
    key=field.get("storage",{}).get("key","")
    group=key.split(".",1)[0].split("[",1)[0] if "." in key or "[" in key else "ConfigRoot"
    groups[group].append(field["id"])
for n,(group,ids) in enumerate(sorted(groups.items()),1):
    suffix=f".S{n:02d}"
    (OUT/"tasks"/f"R4-13{suffix}.md").write_text(render(SPECS[13],suffix,ids,group),encoding="utf-8")
    instances.append({"task":"R4-13"+suffix,"parameter":group,"ledger_ids":ids})
for parent,params,prefix in [
    ("18",["VMess","VLESS","Shadowsocks","SOCKS","HTTP","Trojan","Hysteria2","TUIC","WireGuard","AnyTLS","NaiveProxy"],"P"),
    ("33",["Windows x64/WPF","Windows ARM64/WPF","macOS x64/Avalonia","macOS ARM64/Avalonia","Linux x64/Avalonia","Linux ARM64/Avalonia"],"O")
]:
    for n,param in enumerate(params,1):
        suffix=f".{prefix}{n:02d}"
        (OUT/"tasks"/f"R4-{parent}{suffix}.md").write_text(render(SPECS[int(parent)],suffix,parameter=param),encoding="utf-8")
        instances.append({"task":"R4-"+parent+suffix,"parameter":param})
manifest={"application_commit":BASE,"upstream_commit":FROZEN,"status":"identified","production_edits":False,"work_packages":len(SPECS),"parameter_cards":len(instances),"coverage_rows":len(rows),"note":"参数卡是覆盖实例，不是完成度分母；仍逐核/平台适用性执行。","inventories":dict(collections.Counter(x["ledger"]+":"+x["collection"] for x in rows)),"instances":instances}
(OUT/"execution-manifest.json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
index=["# 完整移植修复执行卡","","状态：identified。先读总方案与领域报告，再按前置和文件唯一写入者执行。","","| 卡 | 用户流程 | 前置 |","|---|---|---|"]
for spec in SPECS: index.append(f"| [R4-{spec[0]}](R4-{spec[0]}.md) {spec[1]} | {spec[2]} | {spec[3]} |")
index+=["","参数卡，每张一个分组/协议/平台实例：",""]
for item in instances: index.append(f"- [{item['task']}]({item['task']}.md)：{item['parameter']}")
index+=["","R4-13/18/33为协调包，修改用实例卡；共享文件排队，不能并行抢写。单实例通过不能带过其余实例。","","全条目对应 ../coverage.csv，参数索引 ../execution-manifest.json。任务卡文件数不代表完成度。"]
(OUT/"tasks"/"README.md").write_text("\n".join(index)+"\n",encoding="utf-8")
print(json.dumps({k:manifest[k] for k in ["work_packages","parameter_cards","coverage_rows"]},ensure_ascii=True))

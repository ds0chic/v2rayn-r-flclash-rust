"""Build the plan inventory, without running product code or tests."""
import csv
import json
from collections import Counter
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).with_name("SETTINGS_IMPLEMENTATION_180.csv")
SPEC = ROOT / "compat/fields.settings.yaml"
AUDIT = ROOT / "docs/evidence/complete-port-audit-2026-10-06/settings/settings-current-180.csv"

items = yaml.safe_load(SPEC.read_text(encoding="utf-8"))["items"]
with AUDIT.open(encoding="utf-8-sig", newline="") as f:
    current = {row["id"]: row for row in csv.DictReader(f)}

plans = {}


def set_plan(n, owner, consumer, verification, dependencies="SD-01/02/04; 主方案正式窗口与命令队列", related="", research="无需新架构研究；尚需正式链路验证", gap=""):
    if n in plans:
        raise ValueError(f"Duplicate plan row {n}")
    plans[n] = {
        "owner_work_package": owner,
        "related_work_packages": related,
        "final_consumer": consumer,
        "required_verification": verification,
        "dependencies": dependencies,
        "research_status": research,
        "extra_gap": gap,
    }


ROUNDTRIP = "正式入口→真实Rust提交→JSON/SQLite一致修订→退出重开；"
CODEGEN = ROUNDTRIP + "同revision正式runtime plan→对应core配置校验/真实会话；保存/字段赋值不算效果验证"

set_plan(1, "SD-03", "application默认节点/启动决策读取canonical IndexId；backup ID remap与activation；非表格focus或applied_target_id", ROUNDTRIP + "选B启动/停机重开/原版ZIP与native ZIP导入后仍指B；真实backup_service B→A反例回归", "SD-01/02; 节点四身份合同; SD-17", "SD-04/06/17", gap="active_index_id元数据与canonical IndexId已确认不一致；不得依靠UI回写修复")
set_plan(2, "SD-03", "application当前订阅组canonical SubIndexId；profiles读取统一snapshot；backup导入remap", ROUNDTRIP + "多组选择/删除选中组/导入重编号/重开仍选正确组；空组与不存在ID冻结回退", "SD-01/02; 订阅事务/ID remap; SD-17", "SD-06/17")
for n in range(3, 26):
    set_plan(n, "SD-01", "严格raw文档组校验/保留；typed view供该组叶子消费者；容器本身没有新增用户功能", "该组missing/null/empty/bad type/未知键保留；组patch不覆盖其它组；子字段按各自ID验收", "逐ID冻结schema/default/nullable; SD-02可恢复提交", "SD-02/04", gap="容器只计台账覆盖，不计一项真实开关；整树unwrap_or_default必须移除")

general = {
    26: ("application::settings_to_codegen→core log启停", "CoreBasic.LogEnabled关/开：真实core log文件及level行为；不等同应用EnableLog"),
    27: ("application::settings_to_codegen→xray/sing-box/core log.level", "各合法level、错误enum与关闭log组合；真实core日志级别"),
    28: ("outbound TLS/reality默认fingerprint回退；节点显式值按冻结优先级", "默认/节点显式值/不支持core组合输出和真实握手，未知值按冻结处理"),
    29: ("outbound HTTP/WS等默认User-Agent/header生成，节点显式header优先", "空/自定义/节点覆盖，多transport真实请求header；凭据不入证据"),
    30: ("xray outbound.sendThrough等适用core出站绑定", "合成可用/不可用本地地址、空值回退；失败不宣称已应用"),
    31: ("sing-box outbound bind_interface等平台适用出站接口绑定", "实际接口选择与不存在接口错误；按core能力判定非适用"),
    32: ("xray/sing-box适用fragment outbound与proxy routing生成", "开/关/节点显式组合、正式core校验与实际target；SD-09合法参数"),
    33: ("适用core final fragment路径及路由收尾生成", "EnableFragment×EnableFinalFragment×TLS场景真值组合；保留冻结覆盖顺序"),
    34: ("sing-box experimental cache_file.enabled及受控data路径", "开/关真实cache file创建/重开行为；不把旧缓存存在当本次启用事实"),
    35: ("inbound socks/http/mixed实际listen端口；runtime ready与平台actual endpoint", "范围1..65535与双端口偏移；安全≥11808探测后监听/端口占用回滚"),
    36: ("各core inbound protocol/type及端口偏移选择", "冻结内部协议合法值、混合/分开入站；真实socks与HTTP请求；不是新协议开关"),
    37: ("inbound UDP能力与实际代理UDP session", "开关在支持core生效；合成UDP往返/拒绝/关闭，TCP不受损"),
    38: ("xray sniffing.enabled / sing-box inbound或route sniff动作", "sniff开关×routeOnly×destOverride配置及合成DNS/HTTP/SNI实际路由"),
    39: ("xray sniffing.destOverride等适用core列表", "冻结列表顺序/未知项策略；实际协议识别不只序列化"),
    40: ("xray sniffing.routeOnly与适用sing-box等价路由语义", "开启仅路由不改目的地址；合成分流对照，关时按冻结core语义"),
    41: ("inbound listen地址/远程LAN访问权限", "loopback/all-interface切换；隔离环境真实LAN可达性与拒绝，不改宿主网络"),
    42: ("LAN专用第二入站/端口分配，按冻结AllowLANConn组合", "AllowLANConn×NewPort4LAN×第二端口组合；实际分配/冲突/停止清理"),
    43: ("LAN inbound认证账户生成/连接认证", "用户名为空/非空×密码组合；正确认证成功错误拒绝；不得打印值"),
    44: ("LAN inbound认证密码生成/secret存取", "正确/错误认证、清空与备份恢复；日志/receipt不包含密码"),
    45: ("application local port resolver→第二本地入站与实际endpoint", "启用/关闭各协议第二端口及偏移；平台endpoint取真实端口不能硬编码"),
    46: ("xray mKCP mtu生成", "冻结单位/范围/合法边界、对应mKCP版本真实校验；新版不兼容明确能力诊断"),
    47: ("xray mKCP tti生成", "冻结单位/范围及core版本实际接受；错误保留旧计划"),
    48: ("xray mKCP uplinkCapacity生成", "字段边界/缺省、方向单位正确；真实支持版本会话"),
    49: ("xray mKCP downlinkCapacity生成", "字段边界/缺省、方向单位正确；真实支持版本会话"),
    50: ("xray mKCP cwnd multiplier适用wire能力映射", "冻结core参数名/版本，合法数值输出与actual validation；不虚称当前新版都支持"),
    51: ("xray mKCP max sending window适用wire能力映射", "冻结core参数名/版本，合法数值输出与actual validation；不支持版本显式诊断"),
    52: ("xray gRPC idle_timeout及适用core映射", "单位/缺省/合法边界；真实gRPC session及空闲行为"),
    53: ("xray gRPC health_check_timeout及适用core映射", "单位、超时与取消；真实gRPC session；错误core参数不可saved=applied"),
    54: ("xray gRPC permit_without_stream及适用core映射", "布尔组合在实际gRPC transport配置/会话，冻结语义"),
    55: ("xray gRPC initial_windows_size及适用core映射", "单位/零值/合法边界；对应版本校验、窗口参数实际发送路径"),
    59: ("profiles去重服务KeepOlderDedupl选择保留旧/新身份与active/sub关联", "合成重复节点新旧顺序、标签/订阅/当前节点关联及重开；不能仅list去重数量"),
    60: ("application订阅scheduler AutoUpdateInterval真实调度/取消/重新计划", "虚拟clock故障+A；正式打开、改周期、不重叠、退出后停止；失败订阅不覆盖旧组"),
    61: ("DesktopIntegration托盘节点菜单数量上限与当前节点展示", "0/1/上限/大列表、实际native tray菜单顺序/当前标记与快捷切换"),
    84: ("订阅转换器请求构造SubConvertUrl；实际合成输入转换→解析→事务替换", "禁用/空/自定义HTTPS converter/错误响应/取消，不泄漏真实订阅；冻结URL组合与编码"),
    93: ("CoreTypeItem.ConfigType→protocol-to-core绑定键，resolve_target_core", "各协议/默认/未知enum/删改绑定与重开；正式启动选择正确exe不是只序列化"),
    94: ("CoreTypeItem.CoreType→resolve_target_core及core availability/能力校验", "锁定对应core/native配置/缺核提示；当前支持真实会话，不把App=99当proxy core"),
    106: ("Rust speedtest worker超时/cancel deadline", "合成本地慢响应/超时/取消实际worker；选值后重开并正式测速不悬挂"),
    107: ("Rust速度测试真实下载请求SpeedTestUrl，shared HttpPolicy", "合成本地定长下载/重定向/错误内容/取消；真实字节速度，禁止把ping当下载测速"),
    108: ("Rust delay/ping worker HTTP target；正式codegen speedPing URL上下文", "本地响应、超时、错URL、选定节点真实代理请求；分清TCP与HTTP delay"),
    109: ("测速任务队列MixedConcurrencyCount资源/进程限流", "N=1/N>1峰值实际worker数与取消释放；UI批量结果/原选择保留，不只改配置"),
    110: ("IP检查HttpPolicy请求/JSONPath解析IPAPIUrl", "合成正确/坏schema/IP与geo字段/错误URL；实际UI显示；不读取用户节点"),
    111: ("Rust UDP test target parser→对应core真实UDP会话", "合成UDP回显server、端口范围/超时/取消；结果分类准确"),
    112: ("速度测试任务分页/批次规模SpeedTestPageSize", "多页节点完整结果/边界/空页/取消；不得漏节点或重复启动"),
    113: ("测速调度SpeedTestDelayInterval间隔deadline", "虚拟clock约束+A；实际任务间隔/取消及界面响应性，不阻塞UI线程"),
    114: ("xray routing.domainStrategy默认与routing实体显式优先级", "设置值/活动路由值组合冻结优先级；真实DNS和分流；旧RoutingIndexId见SD-15"),
    115: ("sing-box routing domain strategy/default resolver映射", "按冻结core版本语义/当前能力，不向不支持wire发旧字段；真实解析/分流"),
    120: ("xray mux concurrency生成", "合法边界/禁用mux值及TLS transport互斥；真实core validation/session"),
    121: ("xray mux xudpConcurrency生成", "边界/禁用组合与实际UDP路径；缺core支持显式诊断"),
    122: ("xray mux xudpProxyUDP443枚举生成", "reject/allow/skip等冻结值、错误enum；合成UDP443分流"),
    123: ("sing-box multiplex.protocol生成", "冻结协议枚举与core支持；真实validation/session"),
    124: ("sing-box multiplex.max_connections生成", "零/范围边界/并发限制与真实session"),
    125: ("sing-box multiplex.padding生成", "开/关mux组合、支持transport真实session"),
    126: ("Hysteria适用版本up_mbps生成及各core能力映射", "单位/零值/缺省；实际对应Hysteria版本会话，不跨Hysteria2臆造兼容"),
    127: ("Hysteria适用版本down_mbps生成及各core能力映射", "单位/零值/缺省；实际对应版本会话"),
    128: ("Hysteria port hopping interval生成/端口列表配合", "interval格式/单位/无hop/有hop实际会话；当前core能力诊断"),
}
for n, (consumer, verify) in general.items():
    related = "SD-09" if n in (32, 33) else "SD-10" if n in (84, 107, 108, 110) else "SD-03" if n == 59 else "SD-05" if n in (35, 45) else ""
    set_plan(n, "SD-07", consumer, CODEGEN + "；" + verify if n not in (59, 60, 61, 84, 106, 107, 109, 110, 111, 112, 113) else ROUNDTRIP + verify, related=related)

set_plan(56, "SD-05", "platform实际autostart Run项/TaskScheduler及AutoRun desired→fact重试", ROUNDTRIP + "合成OS写失败→load→retry第二次仍执行；隔离Windows真实标准/提权原版方式与关闭", "SD-04; autostart actual fact/ownership adapter; 隔离OS验收", "SD-18", gap="_autostartApplied在load用desired回填可屏蔽失败重试；冻结admin计划任务方式需对照")
set_plan(57, "SD-13", "monitor/core API真实统计采集开关与表格统计列", ROUNDTRIP + "重启应用时启用/禁用，真实API采集/流量、计数聚合与列显隐；关闭不伪造零", related="SD-07/06")
set_plan(58, "SD-13", "monitor真实速率展示/托盘状态/主窗口，按冻结启动时机", ROUNDTRIP + "实际流量→delta/rate→显示；关后对应速率展示关闭，不只修改Dart bool", related="SD-07/06")
set_plan(62, "SD-11", "所有Windows Flutter engine创建前的真实renderer policy，SoftwareOnly/HW模式事实", "锁定engine源码可行性→最小release实验→所有主/独立窗真实renderer/GPU观察与帧耗时；开关与重开", "Flutter3.47.5 engine/embedder锁定研究; SD-01启动设置; 主方案多窗口", "SD-06/18", "需研究：该层无SoftwareOnly直接开关；须查锁定engine与release实验，Impeller禁用不等同软件渲染", "现有仅保存；不能伪映射低功耗GPU/禁Impeller完成")
set_plan(63, "SD-12", "Rust/host/UI应用诊断logger初始化、轮转、禁用真实写入；不是core LogEnabled", ROUNDTRIP + "restart_app后真实应用日志启停/归属轮转/脱敏；恢复journal仍能恢复", "SD-01/02; logger初始化与归属清理", "SD-07", gap="EnableLog尚无对应应用logger最终消费者")
set_plan(64, "SD-10", "共享HttpPolicy/ClientFactory→所有应用HTTPS trust roots system/chrome/mozilla", ROUNDTRIP + "A/B公开合成CA选择接受/拒绝、hostname/过期/不可信拒绝；订阅/更新/WebDAV/路由/Geo逐真实client；不装OS证书", "锁定TLS backend/root bundle来源版本hash; 各网络模块注入policy", "SD-07/15/16/17", "需核定具体TLS backend与bundle实现；无OS证书安装需求", "cert mapping有代码，但正式reqwest客户端未接provider")
set_plan(65, "SD-13", "日志服务/LogsView canonical MainMsgFilter，冻结regex/过滤表达式语义", ROUNDTRIP + "正确/非法表达式、日志实时/暂停/复制；与冻结MsgViewModel显示一致", related="SD-06/12", gap="当前局部keyword状态不是canonical MainMsgFilter")
set_plan(66, "SD-13", "日志订阅消费/LogsView canonical AutoRefresh与生命周期", ROUNDTRIP + "关刷新保留缓冲事实、恢复不重复；隐藏/重开按原版；无无限队列", related="SD-06/12", gap="局部auto-refresh与canonical设置未接线")

ui = {
    67: ("主profiles真实列宽计算EnableAutoAdjustMainLvColWidth", "开/关真实列宽变化、100/150/200%DPI，手动宽度/未知列不丢失", "SD-13"),
    68: ("主窗分隔区域高度1canonical→实际splitter布局", "拖动→保存→重开/备份恢复；最小高度/窗口缩放/3布局不拥挤", ""),
    69: ("主窗分隔区域高度2canonical→实际splitter布局", "同布局第二splitter；两值独立，隐藏区恢复不覆盖原值", ""),
    70: ("主窗3布局选择MainGirdOrientation→真实布局生成", "每布局菜单/表/日志/状态位置与冻结一致，按restart_app应用并重开", ""),
    71: ("全窗口ColorPrimaryName主题accent resolver", "合法/未知色名冻结回退、所有独立窗一致，对比度与菜单选中色", ""),
    72: ("全窗口CurrentTheme实际Material/native主题", "light/dark/system原版值、主/独立窗跟随、系统变化与重开", ""),
    73: ("所有Flutter engine locale启动设置与资源fallback", "restart_app后菜单/设置/对话框/独立窗真实文字，不只主窗重绘", ""),
    74: ("所有engine typography启动font family与实际字体fallback", "restart_app、缺字体/CJK、多DPI与独立设置窗；不截断/混文字", ""),
    75: ("全窗口CurrentFontSize typography即时生效", "多字号的行高/点击区/菜单/表头/设置表单布局，实际字体与重开", ""),
    76: ("profiles真实拖动排序能力/顺序持久化，冻结restart_app时机", "同组/跨组/锁订阅/拖动取消/多选择，重开保持正确Id顺序", "SD-03"),
    77: ("profiles double-click→activate命令，行focus/selection不混active", "开启/关闭单击双击/空白/右键路径，启动/失败反馈，旧active事实不变", "SD-03"),
    78: ("DesktopIntegration实际启动隐藏与托盘可恢复窗口", "普通/启动自启场景：隐藏后托盘打开/退出；缺托盘保留可见入口", "SD-18"),
    79: ("DesktopIntegration窗口close生命周期hide-to-tray或真实exit", "两值主窗X→隐藏/退出，独立窗关闭非退出，重开与托盘菜单", "SD-18"),
    81: ("MainColumnItem容器→canonical列Name/Width/Index合并与profiles表", "容器不是新控件；未知列保留，旧column_layout一次迁移/新canonical优先", "SD-03/17"),
    82: ("WindowSizeItem容器→canonical TypeName/Width/Height与原生窗口尺寸", "容器不是新控件；旧INI一次迁移/各window type/坏尺寸回退与备份恢复", "SD-17/18"),
    83: ("profiles列实际HideColumnIpInfo显隐", "开/关IP与geo列/操作列布局；隐藏不删数据；重开/备份恢复", "SD-13"),
    117: ("profiles canonical column Name标识/未知列保存与未来兼容", "内部column身份非用户手输；字段顺序重开/恢复，新版未知列不被清空", "SD-17"),
    118: ("profiles canonical列Width实际像素/DPI映射", "真实resize、autoAdjust开/关、最小宽度/DPI/窗口缩放，重开仍正确", "SD-13/17"),
    119: ("profiles canonical列Index真实显示顺序", "实际列重排→保存→重开/ZIP恢复；重复/坏Index冻结处理与未知列", "SD-17"),
    136: ("connections真实列canonical layout数组及表组件", "每列真实resize/order/隐藏/重开/备份恢复；容器不冒称额外功能", "SD-13/17"),
    156: ("原生窗口canonical TypeName→对应窗口geometry记录", "内部类型不是用户手输；主/设置/路由等冻结window type，独立记录不覆盖", "SD-18/17"),
    157: ("各原生窗口canonical Width→实际窗口尺寸", "resize/save/reopen，工作区/DPI安全clamp，旧INI迁移与backup往返", "SD-18/17"),
    158: ("各原生窗口canonical Height→实际窗口尺寸", "resize/save/reopen、最小内容高、DPI安全clamp，独立窗口与backup往返", "SD-18/17"),
}
for n, (consumer, verify, related) in ui.items():
    set_plan(n, "SD-06", consumer, ROUNDTRIP + verify, "SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移", related)

set_plan(80, "SD-18", "macOS正式runner/window-manager activation policy show/hide Dock", ROUNDTRIP + "macOS release真实Dock显示/隐藏、托盘恢复与启动；Windows按冻结不适用", "macOS真实构建/运行; SD-06窗口policy", "SD-06", gap="当前保留字段，无实际Dock activation consumer")
for n, effect in {85:"GeoSourceUrl的{0}=geoip/geosite资源下载→verified文件→正式codegen上下文",86:"SrsSourceUrl的{0}=type/{1}=file下载→srss/manifest→local_srs_files snapshot",87:"RouteRulesTemplateSourceUrl实际HTTPS模板job→schema校验→路由候选提交/刷新UI",116:"legacy RoutingIndexId→源ID remap→目标IsActive→正式活动routing选择，迁移后清除旧键"}.items():
    set_plan(n, "SD-15", effect, ROUNDTRIP + ("旧ID指定第二路由的load/native/原版ZIP导入后实际生成选第二路由" if n == 116 else "本地合成资源源/坏内容/中断取消/离线；正式计划实际消费下载内容，SRS不继续暗访问remote"), "SD-01/02; SD-10client; routing/resource source schema; 正式codegen context", "SD-07/10/17", gap="只落下载文件/保存URL不算效果；必须在正式plan实际接入" if n != 116 else "旧键迁移当前缺失，普通set_default不替代迁移")

for n, field in {88:"EGlobalHotkey动作集合",89:"Alt",90:"Control",91:"Shift",92:"KeyCode"}.items():
    set_plan(n, "SD-18", "canonical GlobalHotkeys[] "+field+"→native组合注册/多动作派发与失败旧注册保留", ROUNDTRIP + "冻结同组合多动作顺序、注册冲突/失败/retry；隔离OS只用无害动作真实按键；未授权宿主写入不跑", "SD-04; 真实native hotkey adapter; 原版正式注册入口时机核定", "SD-06", "需核定：台账next_launch与冻结正式设置入口即时注册路径的时机；非消费者缺失", "现有单测/插件路径不等同真实OS同组合多动作已验")

for n, (wire, scenario) in {
    95:("runtime TUN desired模式→唯一host/helper session/lease", "开启/关闭真实TUN生命周期/失败retry/停止清理；界面实际fact而非bool"),
    96:("TUN auto_route→helper真实路由ownership lease", "隔离VM默认路由前后/失败补偿/退出恢复；空闲监听端口不能代替路由隔离"),
    97:("TUN strict_route→对应core TUN wire与真实路由", "隔离VM strict on/off/泄漏与例外地址；停止恢复"),
    98:("TUN stack枚举→对应core/平台实现", "system/gvisor/mixed等冻结支持矩阵、错误能力、隔离真实流量"),
    99:("TUN mtu→core TUN device真实MTU", "边界、实际设备MTU/大包fragment/正常流量；错误保留旧session"),
    100:("TUN IPv6地址启用→IPv6 TUN地址/route", "启用/关闭IPv6设备/路由和泄漏；无IPv6能力准确诊断"),
    101:("TUN icmp routing→core对应版本ICMP动作/路由", "隔离IPv4/IPv6合成ICMP和UDP/TCP互不损坏；版本能力诊断"),
    102:("TUN legacy protect→冻结保护例外/适用core配置", "版本/平台适用与真实自环保护；不是万能绕过失败开关"),
    103:("TUN RouteExcludeAddress→CIDR解析→core排除→真实route bypass", "合法/坏CIDR与多个地址、真实目标绕行；路由清理无残留"),
    104:("TUN IPv4Address列表→设备地址/路由", "合法CIDR/冲突/空/错误输入；隔离实际设备/监听/流量与清理"),
    105:("TUN IPv6Address列表→设备地址/路由", "合法IPv6 CIDR/冲突/无平台能力；隔离实际设备/路由与清理"),
}.items():
    set_plan(n, "SD-07", "settings/codegen输入归本包；最终效果归主方案TUN生命周期包："+wire, CODEGEN + "；"+scenario, "主方案TUN实际session/route lease/退出清理; SD-04/05; 隔离VM真实验收", "SD-05/18", gap="设置保存/codegen覆盖与真实TUN会话必须分列；归属/停机/OS效果不得只用mock证明")

set_plan(129, "SD-14", "Mihomo custom plan正式mixin options ipv6→native YAML/core", CODEGEN + "；enabled/disabled/自带YAML值覆盖顺序与实际IPv6能力", "SD-01/02; 冻结Clash merge顺序; native custom plan", "SD-07", gap="已有mixin helper未接当前native runtime plan")
set_plan(130, "SD-14", "Mihomo custom plan EnableMixinContent→实际受控mixin文件merge/关闭保留文件", CODEGEN + "；关/开/不存在/坏YAML/未知键，真实API与配置效果；关闭不删原文件", "SD-01/02; SD-10可选资源; 冻结Clash merge顺序", "SD-07", gap="helper单测不替代正式custom plan merge")
for n, field in {131:"proxies sorting",132:"proxies auto refresh",133:"proxies refresh interval",134:"connections auto refresh",135:"connections refresh interval"}.items():
    set_plan(n, "SD-13", "canonical ClashUI "+field+"→真实core API轮询/排序、可见页面生命周期", ROUNDTRIP + "实际Clash API数据/顺序或请求频率、隐藏暂停/取消、core切换；保存失败可见且回滚用户选择", "SD-04; native monitor端点来自实际session; timer lifecycle", "SD-14", gap="保存异步失败传播需统一；不能只看当前UI值变化")

for n, wire in {137:"SysProxyType→actual模式/endpoint/ownership apply",138:"SystemProxyExceptions→实际bypass列表",139:"NotProxyLocalAddress→实际local bypass合成",140:"SystemProxyAdvancedProtocol→实际protocol-map代理值",141:"CustomSystemProxyPacPath→原版PAC读文件/默认fallback/内容hash/服务实际文件"}.items():
    set_plan(n, "SD-05", "platform service "+wire+"，仅成功事实可去重", ROUNDTRIP + "fake仅失败注入；同mode/session/port改内容仍apply/retry；隔离Windows查询真实proxy/PAC值与恢复归属", "SD-04; actual Endpoint/PlatformReceipt/ownership ledger; 隔离OS场景", "SD-07/18", gap="现去重key只有mode/session/port，遗漏配置内容与PAC实际内容hash")
set_plan(142, "SD-18", "macOS/Linux frozen proxy adapter实际执行CustomSystemProxyScriptPath及安全参数/timeout/exit/补偿", ROUNDTRIP + "合成脚本参数/cwd/成功/失败/超时/取消；平台真实效果与恢复；Windows按原版不适用", "SD-05; 冻结ProxySettingLinux/OSX; 独立平台release", "SD-10", gap="当前只有路径有效性校验，不能称脚本真实生效")

for n, field in {143:"Url",144:"UserName",145:"Password",146:"DirName"}.items():
    set_plan(n, "SD-17", "WebDAV "+field+"→shared HttpPolicy真实PROPFIND/PUT/GET资源路径/认证→受控backup/restore", ROUNDTRIP + "合成本地WebDAV真实协议：编码/目录/auth/失败/重试/中断；ZIP/native/原版bundle canonical IDs/未知键往返；secret不入日志", "SD-01/02/03; SD-10; actual backup lifecycle service", "SD-04/16", gap="mock HTTP/ZIP解析不证明真实WebDAV+DB+UI重开；password处理按现有秘密保护合同")

for n, field in {147:"prerelease channel",148:"via_proxy当次实际session/policy",149:"SelectedCoreTypes冻结auto目标集合"}.items():
    set_plan(n, "SD-16", "update check/apply explicitflags "+field+"；应用真实发行repo/pubkey；四auto目标与14manual库存区分", ROUNDTRIP + "保存失败可见不继续；当次flags进入metadata/asset；真实发行签名/错签/篡改/runner交接/安装失败与数据兼容回滚", "本项目真实release repo/资产/可信公钥；SD-04/10；签名格式/runner/数据兼容区间", "SD-17", "产品前置：发行repo/publickey/渠道；需核定签名发行格式与真实安装路径", "Rust with_flags存在但UI/BridgePort未接；app_repo=None；签名来源不能继续冒用上游应用身份")

for n, field in {150:"Packets",151:"Lengths",152:"Delays",153:"MaxSplit",154:"legacy Length",155:"legacy Interval"}.items():
    verify = "MaxSplit单值/范围1-3/0/10000/倒序/负数/多段，冻结wire只取first整数；raw范围保留" if n == 153 else "冻结字符串格式/默认/legacy迁移与EnableFragment/FinalFragment组合；UI/Rust判定一致"
    set_plan(n, "SD-09", "raw Fragment4Ray "+field+"→统一parse/default→xray/sing-box适用outbound；legacy值仅迁移源", CODEGEN + "；"+verify, "SD-01/07; 冻结Utils.TryParseMaxSplit与V2rayOutboundService; core版本能力", "SD-08", gap="MaxSplit范围已确认被当前后端拒绝" if n == 153 else "legacy内部迁移项不是新可见开关；需要raw/default兼容")

dns = {
    159:("UseSystemHosts→core DNS hosts/system hosts注入", "开/关真实系统hosts目标解析；不读/记录用户秘密hosts内容"),
    160:("AddCommonHosts→冻结公用hosts表merge优先级", "公用表启停与自定义Hosts冲突顺序、真实解析"),
    161:("FakeIP→适用core DNS fakeip与rule结构", "开/关DNS响应及路由，和GlobalFakeIp/TUN组合"),
    162:("GlobalFakeIp→DNS/routing全局fakeIP覆盖范围", "全局/非全局与例外目标、真实DNS响应与连接"),
    163:("FakeIPRange→DNS fakeip pool CIDR", "合法/坏IPv4/IPv6 CIDR及范围实际响应；core版本支持"),
    164:("BlockBindingQuery→DNS bindingquery阻断策略", "合成绑定查询阻断/不阻断，非目标普通查询正常"),
    165:("BlockAAAAQuery→真实AAAA DNS阻断策略", "AAAA开关与IPv4正常/IPv6启用组合，不能仅移除UI选项"),
    166:("DirectDNS→直连DNS server列表及bootstrap", "本地双DNS server分辨实际请求目的，错误/空列表冻结fallback"),
    167:("RemoteDNS→代理DNS server与proxy detour", "真实经选定节点DNS请求、bootstrap无环、故障旧计划保留"),
    168:("BootstrapDNS→域名DNS server地址bootstrap resolver", "合成需要bootstrap的DNS域名、无环请求、IP server差异"),
    169:("Strategy4Freedom→直连出站domain strategy", "冻结枚举、实际IPv4/IPv6优先/解析行为，不能只DNS config"),
    170:("Strategy4Proxy→代理DNS/outbound domain strategy", "冻结策略与Happy开关/IPv6组合的实际解析/出站"),
    171:("Strategy4ProxyDial→proxy dial解析策略", "代理server域名dualstack/失败fallback、对应core实际dial"),
    172:("ServeStale→适用core DNS缓存过期响应策略", "合成TTL与上游故障、stale on/off；当前core能力校验"),
    173:("ParallelQuery→适用core DNS并行query", "合成两DNS不同延迟、实际并发/结果选择与取消"),
    174:("Hosts→原始用户hosts parser/merge→DNS wire", "空/合法/坏格式/IPv6/列表/公用hosts冲突、真实解析与未知raw保留"),
    175:("DirectExpectedIPs→直连DNS expectedIPs/fallback匹配", "冻结geo/CIDR规则、合成符合/不符响应触发正确fallback"),
}
for n, (consumer, verify) in dns.items():
    set_plan(n, "SD-07", "SimpleDNS "+consumer, CODEGEN+"；"+verify, "SD-01/02; 活动DNS/routing方案优先级; core版本能力", "SD-08/15")
for n, field in {176:"EnableHappyEyeballs gating",177:"TryDelayMs",178:"PrioritizeIPv6",179:"Interleave",180:"MaxConcurrentTry"}.items():
    set_plan(n, "SD-08", "冻结SimpleDNS/HappyEyeballs "+field+"→xray DNS/sockopt happyEyeballs，仅enable且非skip路径输出", CODEGEN + "；false不输出参数/true输出；skip×enable真值表；合成dualstack失败/延迟、实际参数行为", "SD-07 DNS策略/IPv6; 冻结V2rayDnsService enabled/skip分支", "SD-07", gap="false当前仍输出happyEyeballs且开关配置相同，已有实际Rust反例")

columns = ["id", "row_kind", "canonical_path", "owner_work_package", "related_work_packages", "final_consumer", "apply_timing", "required_verification", "dependencies", "current_gap", "research_status", "platform_scope", "original_type", "original_default", "source_file", "source_symbol", "source_commit"]
rows = []
for item in items:
    n = int(item["id"].rsplit("-", 1)[1])
    plan = plans[n]
    audit = current[item["id"]]
    gaps = [audit["current_note"], plan["extra_gap"]]
    rows.append({
        "id": item["id"],
        "row_kind": "internal" if n <= 2 else "container" if n <= 25 else "leaf",
        "canonical_path": item["storage"]["key"],
        **{key: plan[key] for key in ("owner_work_package", "related_work_packages", "final_consumer", "required_verification", "dependencies", "research_status")},
        "apply_timing": item["apply_timing"] + "（冻结台账；保存成功与实际应用阶段分开）",
        "current_gap": "；".join(gap for gap in gaps if gap) + "；本行尚未完成逐字段正式入口/最终效果验收",
        "platform_scope": item["platform_scope"],
        "original_type": item["type"],
        "original_default": json.dumps(item["default_value"], ensure_ascii=False, separators=(",", ":")),
        "source_file": item["source_file"],
        "source_symbol": item["source_symbol"],
        "source_commit": item["source_commit"],
    })

assert len(rows) == 180
assert len(plans) == 180
assert len({row["id"] for row in rows}) == 180
assert {row["id"] for row in rows} == {item["id"] for item in items}
assert Counter(row["row_kind"] for row in rows) == {"container": 23, "internal": 2, "leaf": 155}
with OUT.open("w", encoding="utf-8-sig", newline="") as f:
    writer = csv.DictWriter(f, fieldnames=columns)
    writer.writeheader()
    writer.writerows(rows)
print(json.dumps({"output": str(OUT), "rows": len(rows), "row_kinds": dict(Counter(row["row_kind"] for row in rows)), "owners": dict(Counter(row["owner_work_package"] for row in rows))}, ensure_ascii=False))

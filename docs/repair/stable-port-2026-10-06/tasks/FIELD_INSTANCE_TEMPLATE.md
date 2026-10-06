# SP-23 字段实例卡模板

状态：identified；模板本身不是字段完成证据。从../SETTINGS_IMPLEMENTATION_180.csv选一个真实ID，复制到新卡并填写所有实例值；不得覆盖已有卡。

任务 ID：SP-23.<实际FLD-CFG-ID>（若是某SP-24/25/26的专属消费者可同时关联该整合卡，保持唯一主owner）

本次唯一用户流程：<该字段canonical_path的原版入口→一次操作→对应实际消费者；容器为读取/迁移其结构，内部字段为相应identity流程，不造控件>

前置任务及已验证证据：SP-00、SP-01/02、SP-12和CSV本行dependencies；涉及运行需SP-04..07，涉及平台需SP-15及授权隔离验收。写实际证据路径和未验证，不从模板假设前置完成。

对应 feature / field / action / layout ID：<本行精确id>；<真实关联ID及来源>。填row_kind/container/internal/leaf、platform_scope、original_type/default；共用consumer可复用证据但每ID断言可区分。

必读上游文件、符号和固定 commit：从本行source_file/source_symbol复制并查真实冻结源码；固定7d6a967c18c697f28dc6917122ed3a4993fcf336。当前HEAD记录完整SHA，读取CSV owner SD工作包分册、主卡和现存UI/Rust consumer。CSV生效时机是冻结台账来源，若正式入口与台账冲突必须核原版真实调用链并登记，不任选方便时机。

输入、输出、错误、取消、权限、持久化及生效语义：<合法值/边界/null-empty-missing/非法值、UI草稿/提交、newRevision和保存/core/platform阶段、final_consumer、apply_timing、重开/重启>。取消提交前不写，已提交后不假称无效果；unknown先query；保存失败不更新applied。无平台副作用在宿主可用合成数据测；平台写入只授权隔离机，10808禁占，测试端口≥11808预探测。

允许修改的模块：<实际消费者文件和符号；不列整个workspace无限授权>；正确合同/本卡/证据。与共享文件writer预约，FRB一人生成。未知raw键、数据面/权限边界保留。

禁止改变的已有行为：<原版同组其它设置及已修行为>；原版nullable/enum/default/生效时机，未知键保留，stale拒绝，保存/运行分离；不削台账，不把container当开关。

测试夹具和原版预期：<synthetic输入、core/version/TLS/OS条件>；正向两个不同有效值，负向边界/坏类型/持久化或consumer故障，取消和并发stale；同输入原版预期有源引用或真实对照。真实FRB/磁盘/独立重开/最终consumer；fake仅故障注入。

本次必须通过的命令/真实场景：按改动运行对应cargo test -p <实际package> --locked，fmt/clippy；apps/desktop下flutter analyze及新增行为测试；native改动先release构建。填确切测试文件/名称，不能留占位宣称通过。<本行required_verification具体操作和观察>；最终完整门禁SP-34。

证据文件位置：docs/evidence/stable-port/SP-23/<精确ID>/；metadata绑定commit/package/core/system/renderer与DPI。尚未实测的平台写未验证，真实日志不含秘密。

完成条件：保存、重开和最终消费者确实改变，失败可恢复，原版时机不改；本行正负向完整。容器仅结构/未知键/迁移闭环，不能替子字段生效；内部身份须真实备份/恢复/重开用例。有纯库证据但缺正式UI/consumer只implemented。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。写提供方/调用方/拟DTO/错误/取消/版本/存储/生效点和writer，保持原ID等待整合。

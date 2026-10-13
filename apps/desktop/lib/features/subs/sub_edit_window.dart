import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

/// The subscription editor (upstream `SubEditWindow`, F-SUB-002/004/005/006/
/// 007/008/009). Edits all 17 `SubItem` fields and validates before saving.
///
/// Returns the edited draft on save, or `null` when cancelled (nothing is
/// persisted: the caller owns the save).
Future<c.SubItemDto?> showSubEditWindow(
  BuildContext context,
  c.SubItemDto initial,
) {
  return showDialog<c.SubItemDto>(
    context: context,
    barrierDismissible: false,
    builder: (_) => SubEditWindow(initial: initial),
  );
}

class SubEditWindow extends ConsumerStatefulWidget {
  const SubEditWindow({super.key, required this.initial});

  final c.SubItemDto initial;

  @override
  ConsumerState<SubEditWindow> createState() => _SubEditWindowState();
}

/// True for loopback / RFC 1918 / link-local hosts (upstream
/// `Utils.IsPrivateNetwork` counterpart). Used only for the http warning below.
bool isPrivateNetworkHost(String host) {
  final h = host.trim().toLowerCase();
  if (h.isEmpty) return false;
  if (h == 'localhost' || h == '::1' || h == '[::1]') return true;
  final v4 = RegExp(r'^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$')
      .firstMatch(h);
  if (v4 != null) {
    final o = [1, 2, 3, 4].map((i) => int.parse(v4.group(i)!)).toList();
    if (o.any((e) => e > 255)) return false;
    if (o[0] == 10) return true;
    if (o[0] == 172 && o[1] >= 16 && o[1] <= 31) return true;
    if (o[0] == 192 && o[1] == 168) return true;
    if (o[0] == 127) return true;
    if (o[0] == 169 && o[1] == 254) return true;
    return false;
  }
  return false;
}

class _SubEditWindowState extends ConsumerState<SubEditWindow> {
  late final TextEditingController _remarks;
  late final TextEditingController _url;
  late final TextEditingController _moreUrl;
  late final TextEditingController _userAgent;
  late final TextEditingController _requestHeaders;
  late final TextEditingController _filter;
  late final TextEditingController _autoUpdateInterval;
  late final TextEditingController _convertTarget;
  late final TextEditingController _prevProfile;
  late final TextEditingController _nextProfile;
  late final TextEditingController _preSocksPort;
  late final TextEditingController _memo;
  late final TextEditingController _customCoreType;
  late bool _enabled;

  String? _errorText;

  @override
  void initState() {
    super.initState();
    final s = widget.initial;
    _remarks = TextEditingController(text: s.remarks);
    _url = TextEditingController(text: s.url);
    _moreUrl = TextEditingController(text: s.moreUrl);
    _userAgent = TextEditingController(text: s.userAgent);
    _requestHeaders = TextEditingController(text: s.requestHeaders ?? '');
    _filter = TextEditingController(text: s.filter ?? '');
    _autoUpdateInterval = TextEditingController(
      text: s.autoUpdateInterval == 0 ? '' : '${s.autoUpdateInterval}',
    );
    _convertTarget = TextEditingController(text: s.convertTarget ?? '');
    _prevProfile = TextEditingController(text: s.prevProfile ?? '');
    _nextProfile = TextEditingController(text: s.nextProfile ?? '');
    _preSocksPort = TextEditingController(
      text: s.preSocksPort?.toString() ?? '',
    );
    _memo = TextEditingController(text: s.memo ?? '');
    _customCoreType = TextEditingController(
      text: s.customCoreType?.toString() ?? '',
    );
    _enabled = s.enabled;
  }

  @override
  void dispose() {
    for (final c in <TextEditingController>[
      _remarks,
      _url,
      _moreUrl,
      _userAgent,
      _requestHeaders,
      _filter,
      _autoUpdateInterval,
      _convertTarget,
      _prevProfile,
      _nextProfile,
      _preSocksPort,
      _memo,
      _customCoreType,
    ]) {
      c.dispose();
    }
    super.dispose();
  }

  c.SubItemDto _draft() => c.SubItemDto(
    id: widget.initial.id,
    remarks: _remarks.text.trim(),
    url: _url.text.trim(),
    moreUrl: _moreUrl.text.trim(),
    enabled: _enabled,
    userAgent: _userAgent.text.trim(),
    requestHeaders: _emptyToNull(_requestHeaders.text),
    sort: widget.initial.sort,
    filter: _emptyToNull(_filter.text),
    autoUpdateInterval: int.tryParse(_autoUpdateInterval.text.trim()) ?? 0,
    updateTime: widget.initial.updateTime,
    convertTarget: _emptyToNull(_convertTarget.text),
    prevProfile: _emptyToNull(_prevProfile.text),
    nextProfile: _emptyToNull(_nextProfile.text),
    preSocksPort: int.tryParse(_preSocksPort.text.trim()),
    memo: _emptyToNull(_memo.text),
    customCoreType: int.tryParse(_customCoreType.text.trim()),
  );

  String? _emptyToNull(String value) {
    final trimmed = value.trim();
    return trimmed.isEmpty ? null : trimmed;
  }

  void _save() {
    final draft = _draft();
    final controller = ref.read(subsControllerProvider.notifier);
    // Local field checks first, then the authoritative Rust validation.
    if (draft.remarks.isEmpty) {
      setState(() {
        _errorText = '备注不能为空';
      });
      return;
    }
    // An empty URL is a plain group (upstream `SubEditViewModel`): it saves
    // without downloading. A non-empty URL is validated by the Rust bridge
    // (`error.url_invalid`, `E_FIELD_FORMAT`), so the scheme rule has a single
    // source of truth. A non-private `http://` URL only warns (upstream
    // `InsecureUrlProtocol` is likewise non-blocking) and still saves.
    final uri = Uri.tryParse(draft.url);
    if (uri != null &&
        uri.scheme == 'http' &&
        uri.host.isNotEmpty &&
        !isPrivateNetworkHost(uri.host)) {
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('该订阅使用 http 明文传输，请注意安全')));
    }
    final headers = draft.requestHeaders;
    if (headers != null && !headers.trimLeft().startsWith('{')) {
      setState(() {
        _errorText = '请求头必须是 JSON 对象';
      });
      return;
    }
    final bridgeError = controller.validate(draft);
    if (bridgeError != null) {
      setState(() {
        _errorText = bridgeError.messageKey;
      });
      return;
    }
    Navigator.of(context).pop(draft);
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      key: const ValueKey('sub-edit-window'),
      title: Text(
        widget.initial.id.isEmpty ? '新增订阅' : '编辑订阅',
        style: const TextStyle(fontSize: AppTokens.fontSizeDialogTitle),
      ),
      content: SizedBox(
        width: 560,
        height: 480,
        child: SingleChildScrollView(
          child: Column(
            children: <Widget>[
              if (_errorText != null)
                Container(
                  key: const ValueKey('sub-edit-error'),
                  width: double.infinity,
                  padding: const EdgeInsets.all(8),
                  color: Theme.of(context).colorScheme.errorContainer,
                  child: Text(
                    _errorText!,
                    style: TextStyle(
                      fontSize: AppTokens.fontSize,
                      color: Theme.of(context).colorScheme.onErrorContainer,
                    ),
                  ),
                ),
              const SettingsSection(title: '基础', child: <Widget>[]),
              _field(
                '备注 *',
                _remarks,
                'sub-field-remarks',
                help: '给订阅起个名，重名难分。',
              ),
              _field(
                'Url（留空为普通分组）',
                _url,
                'sub-field-url',
                help: '空着＝普通分组，只分类不下载；填远程地址才会更新。',
              ),
              _field(
                '自动更新间隔（分钟，0=禁用）',
                _autoUpdateInterval,
                'sub-field-interval',
                help: '单个订阅多久自动更新一次。0＝手动；家用建议 60–720，太小会被限流。',
              ),
              SwitchListTile(
                key: const ValueKey('sub-field-enabled'),
                dense: true,
                contentPadding: EdgeInsets.zero,
                title: const Text(
                  '启用',
                  style: TextStyle(fontSize: AppTokens.fontSizeSubtitle),
                ),
                subtitle: Text(
                  '关闭后“更新全部”会跳过它。',
                  style: AppForm.helperStyle(Theme.of(context))
                      .copyWith(color: context.semantics.muted),
                ),
                value: _enabled,
                onChanged: (v) => setState(() => _enabled = v),
              ),
              const SettingsSection(title: '进阶', child: <Widget>[]),
              _field(
                'MoreUrl（逗号分隔）',
                _moreUrl,
                'sub-field-moreurl',
                help: '备用更新地址，逗号分隔。',
              ),
              _field(
                'UserAgent',
                _userAgent,
                'sub-field-useragent',
                help: '订阅请求的 UA。留空用默认；乱填会导致 403。',
              ),
              _field(
                '过滤 Filter (正则)',
                _filter,
                'sub-field-filter',
                help: '按节点名过滤的正则。写错会把节点全滤掉，先在别处验证再贴。',
              ),
              _field(
                '转换目标 ConvertTarget',
                _convertTarget,
                'sub-field-convert',
                help: '配合“订阅转换网址”使用；单设无效。与原版差异：原版为下拉，待补。',
              ),
              _field('备注 Memo', _memo, 'sub-field-memo', help: '纯备注，不影响行为。'),
              SettingsSection(
                title: '专家',
                collapsed: true,
                child: <Widget>[
                  const SettingsNote('以下填错可能导致整组不通，教程没让填就别填。'),
                  _field(
                    'RequestHeaders (JSON)',
                    _requestHeaders,
                    'sub-field-headers',
                    maxLines: 2,
                    help: '订阅请求头，必须是 JSON 对象。写错存不上。',
                  ),
                  _field(
                    '前置节点 PrevProfile',
                    _prevProfile,
                    'sub-field-prev',
                    help: '链式代理的前置节点，填节点 id。与原版差异：原版有节点选择按钮，待补；填错整组不通。',
                  ),
                  _field(
                    '后置节点 NextProfile',
                    _nextProfile,
                    'sub-field-next',
                    help: '链式代理的后置节点，填节点 id。与原版差异：原版有节点选择按钮，待补；填错整组不通。',
                  ),
                  _field(
                    '前置 SOCKS 端口',
                    _preSocksPort,
                    'sub-field-presocks',
                    help: '前置 SOCKS 端口。别和前置节点混淆；端口被占会失败。',
                  ),
                  _field(
                    '自定义内核 CustomCoreType',
                    _customCoreType,
                    'sub-field-core',
                    help: '该订阅强制使用的内核编号。与原版差异：原版为下拉，待补；填错对应错内核。',
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('sub-edit-cancel'),
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('sub-edit-save'),
          onPressed: _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Widget _field(
    String label,
    TextEditingController controller,
    String key, {
    int maxLines = 1,
    String? help,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          TextField(
            key: ValueKey(key),
            controller: controller,
            maxLines: maxLines,
            decoration: InputDecoration(
              labelText: label,
              isDense: true,
              border: const OutlineInputBorder(),
            ),
          ),
          if (help != null)
            Padding(
              padding: const EdgeInsets.only(left: 2, top: 2),
              child: Text(
                help,
                style: AppForm.helperStyle(Theme.of(context))
                    .copyWith(color: context.semantics.muted),
              ),
            ),
        ],
      ),
    );
  }
}

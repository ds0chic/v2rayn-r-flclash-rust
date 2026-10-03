import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
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
    // source of truth.
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
        style: const TextStyle(fontSize: 15),
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
                      fontSize: 12,
                      color: Theme.of(context).colorScheme.onErrorContainer,
                    ),
                  ),
                ),
              _field('备注 *', _remarks, 'sub-field-remarks'),
              _field('Url（留空为普通分组）', _url, 'sub-field-url'),
              _field('MoreUrl（逗号分隔）', _moreUrl, 'sub-field-moreurl'),
              _field('UserAgent', _userAgent, 'sub-field-useragent'),
              _field(
                'RequestHeaders (JSON)',
                _requestHeaders,
                'sub-field-headers',
                maxLines: 2,
              ),
              _field('过滤 Filter (正则)', _filter, 'sub-field-filter'),
              _field(
                '自动更新间隔（分钟，0=禁用）',
                _autoUpdateInterval,
                'sub-field-interval',
              ),
              _field('转换目标 ConvertTarget', _convertTarget, 'sub-field-convert'),
              _field('前置节点 PrevProfile', _prevProfile, 'sub-field-prev'),
              _field('后置节点 NextProfile', _nextProfile, 'sub-field-next'),
              _field('前置 SOCKS 端口', _preSocksPort, 'sub-field-presocks'),
              _field('自定义内核 CustomCoreType', _customCoreType, 'sub-field-core'),
              _field('备注 Memo', _memo, 'sub-field-memo'),
              SwitchListTile(
                key: const ValueKey('sub-field-enabled'),
                dense: true,
                contentPadding: EdgeInsets.zero,
                title: const Text('启用', style: TextStyle(fontSize: 13)),
                value: _enabled,
                onChanged: (v) => setState(() => _enabled = v),
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
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: TextField(
        key: ValueKey(key),
        controller: controller,
        maxLines: maxLines,
        decoration: InputDecoration(
          labelText: label,
          isDense: true,
          border: const OutlineInputBorder(),
        ),
      ),
    );
  }
}

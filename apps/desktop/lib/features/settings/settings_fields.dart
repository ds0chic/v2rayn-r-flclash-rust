import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Small form primitives bound to a mutable settings group map. They keep the
/// option window declarative while the document stays a plain JSON map.

class SettingsCheckbox extends StatelessWidget {
  const SettingsCheckbox({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.helpText,
    this.risk,
    this.dirtyKey,
  });

  final String label;
  final bool value;
  final ValueChanged<bool> onChanged;

  /// UB2-01: persistent one-line explainer rendered under the row.
  final String? helpText;

  /// UB2-01: risk note rendered under [helpText] in the warning color.
  final String? risk;

  /// UB2-08: `'Group.Key'` marks the row modified (warning dot after the
  /// label); null renders no dot. Display only, hit areas unchanged.
  final String? dirtyKey;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        InkWell(
          onTap: () => onChanged(!value),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
            child: Row(
              children: <Widget>[
                SizedBox(
                  height: 28,
                  child: Checkbox(
                    value: value,
                    visualDensity: VisualDensity.compact,
                    onChanged: (v) => onChanged(v ?? false),
                  ),
                ),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    label,
                    style: const TextStyle(fontSize: AppTokens.fontSize),
                  ),
                ),
                if (dirtyKey != null) _DirtyDot(dirtyKey: dirtyKey!),
              ],
            ),
          ),
        ),
        _HelpLines(helpText: helpText, risk: risk, indent: 28),
      ],
    );
  }
}

class SettingsTextField extends StatefulWidget {
  const SettingsTextField({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.hint,
    this.width = 240,
    this.enabled = true,
    this.identifier,
    this.helpText,
    this.risk,
    this.dirtyKey,
  });

  final String label;
  final String? value;
  final ValueChanged<String?> onChanged;
  final String? hint;
  final double width;

  /// UB2-01: explainer / risk note under the row (see [SettingsCheckbox]).
  final String? helpText;
  final String? risk;

  /// UB2-08: `'Group.Key'` marks the row modified (see [SettingsCheckbox]).
  final String? dirtyKey;

  /// Stable semantics identifier for UI-automation locators; applied to the
  /// editable field itself so the located node carries the field value.
  final String? identifier;

  /// Upstream gates some text boxes on a linked toggle (e.g.
  /// `togNewPort4LAN` -> `txtuser.IsEnabled`); a disabled box keeps its value.
  final bool enabled;

  @override
  State<SettingsTextField> createState() => _SettingsTextFieldState();
}

class _SettingsTextFieldState extends State<SettingsTextField> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.value ?? '',
  );

  @override
  void didUpdateWidget(covariant SettingsTextField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if ((widget.value ?? '') != _controller.text &&
        widget.value != oldWidget.value) {
      _controller.text = widget.value ?? '';
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              SizedBox(
                width: 168,
                child: Text(
                  widget.label,
                  style: const TextStyle(fontSize: AppTokens.fontSize),
                ),
              ),
              SizedBox(
                width: widget.width,
                child: Semantics(
                  identifier: widget.identifier,
                  child: TextField(
                    controller: _controller,
                    enabled: widget.enabled,
                    decoration: InputDecoration(
                      isDense: true,
                      border: const OutlineInputBorder(),
                      hintText: widget.hint,
                      contentPadding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 8,
                      ),
                    ),
                    style: const TextStyle(fontSize: AppTokens.fontSize),
                    onChanged: (text) =>
                        widget.onChanged(text.isEmpty ? null : text),
                  ),
                ),
              ),
              if (widget.dirtyKey != null)
                _DirtyDot(dirtyKey: widget.dirtyKey!),
            ],
          ),
          _HelpLines(helpText: widget.helpText, risk: widget.risk, indent: 168),
        ],
      ),
    );
  }
}

class SettingsNumberField extends StatefulWidget {
  const SettingsNumberField({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.width = 120,
    this.helpText,
    this.risk,
    this.dirtyKey,
  });

  final String label;
  final int? value;
  final ValueChanged<int?> onChanged;
  final double width;

  /// UB2-01: explainer / risk note under the row (see [SettingsCheckbox]).
  final String? helpText;
  final String? risk;

  /// UB2-08: `'Group.Key'` marks the row modified (see [SettingsCheckbox]).
  final String? dirtyKey;

  @override
  State<SettingsNumberField> createState() => _SettingsNumberFieldState();
}

class _SettingsNumberFieldState extends State<SettingsNumberField> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.value?.toString() ?? '',
  );

  @override
  void didUpdateWidget(covariant SettingsNumberField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.value != oldWidget.value &&
        (widget.value?.toString() ?? '') != _controller.text) {
      _controller.text = widget.value?.toString() ?? '';
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              SizedBox(
                width: 168,
                child: Text(
                  widget.label,
                  style: const TextStyle(fontSize: AppTokens.fontSize),
                ),
              ),
              SizedBox(
                width: widget.width,
                child: TextField(
                  controller: _controller,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    isDense: true,
                    border: OutlineInputBorder(),
                    contentPadding: EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 8,
                    ),
                  ),
                  style: const TextStyle(fontSize: AppTokens.fontSize),
                  onChanged: (text) => widget.onChanged(
                    text.trim().isEmpty ? null : int.tryParse(text.trim()),
                  ),
                ),
              ),
              if (widget.dirtyKey != null)
                _DirtyDot(dirtyKey: widget.dirtyKey!),
            ],
          ),
          _HelpLines(helpText: widget.helpText, risk: widget.risk, indent: 168),
        ],
      ),
    );
  }
}

class SettingsDropdown<T> extends StatelessWidget {
  const SettingsDropdown({
    super.key,
    required this.label,
    required this.value,
    required this.items,
    required this.onChanged,
    this.width = 200,
    this.helpText,
    this.risk,
    this.dirtyKey,
  });

  final String label;
  final T? value;
  final List<DropdownMenuItem<T>> items;
  final ValueChanged<T?> onChanged;
  final double width;

  /// UB2-01: explainer / risk note under the row (see [SettingsCheckbox]).
  final String? helpText;
  final String? risk;

  /// UB2-08: `'Group.Key'` marks the row modified (see [SettingsCheckbox]).
  final String? dirtyKey;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              SizedBox(
                width: 168,
                child: Text(
                  label,
                  style: const TextStyle(fontSize: AppTokens.fontSize),
                ),
              ),
              SizedBox(
                width: width,
                child: DropdownButton<T>(
                  isExpanded: true,
                  isDense: true,
                  value: items.any((i) => i.value == value) ? value : null,
                  items: items,
                  onChanged: onChanged,
                ),
              ),
              if (dirtyKey != null) _DirtyDot(dirtyKey: dirtyKey!),
            ],
          ),
          _HelpLines(helpText: helpText, risk: risk, indent: 168),
        ],
      ),
    );
  }
}

/// Explainer / risk lines shared by the [SettingsCheckbox],
/// [SettingsTextField], [SettingsNumberField] and [SettingsDropdown] rows
/// (UB2-01). Renders nothing when both are null, so existing rows are
/// pixel-identical. [indent] aligns the text with the control column.
class _HelpLines extends StatelessWidget {
  const _HelpLines({this.helpText, this.risk, this.indent = 0});

  final String? helpText;
  final String? risk;
  final double indent;

  @override
  Widget build(BuildContext context) {
    if (helpText == null && risk == null) return const SizedBox.shrink();
    final base = AppForm.helperStyle(Theme.of(context));
    return Padding(
      padding: EdgeInsets.only(left: indent, right: 4, top: 1, bottom: 3),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          if (helpText != null)
            Text(
              helpText!,
              style: base.copyWith(color: context.semantics.muted),
            ),
          if (risk != null)
            Text(risk!, style: base.copyWith(color: context.semantics.warning)),
        ],
      ),
    );
  }
}

/// UB2-08: modified marker shared by the four field rows. A trailing 8px dot
/// in the warning color; the key carries the `'Group.Key'` identity so tests
/// can assert exactly which rows are dirty. Renders after the control, so
/// label width (168) and hit areas are untouched.
class _DirtyDot extends StatelessWidget {
  const _DirtyDot({required this.dirtyKey});

  final String dirtyKey;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(left: 6),
      child: Container(
        key: ValueKey('settings-dirty-$dirtyKey'),
        width: 8,
        height: 8,
        decoration: BoxDecoration(
          color: context.semantics.warning,
          shape: BoxShape.circle,
        ),
      ),
    );
  }
}

/// A labelled sub-section inside a tab.
class SettingsSection extends StatefulWidget {
  const SettingsSection({
    super.key,
    required this.title,
    required this.child,
    this.collapsed = false,
    this.sectionId,
    this.expanded,
    this.onExpandedChanged,
    this.onReset,
    this.dimmed = false,
  });

  final String title;
  final List<Widget> child;

  /// UB2-01: start folded (advanced/legacy groups). Tapping the header
  /// expands; folding never touches persisted values (display only).
  final bool collapsed;

  /// UB2-08: stable id for search jumps (`settings-reset-<id>` key).
  final String? sectionId;

  /// UB2-08: controlled open state. Null keeps the UB2-01 internal toggle.
  final bool? expanded;

  /// UB2-08: called with the new open state when controlled.
  final ValueChanged<bool>? onExpandedChanged;

  /// UB2-08: per-group "restore defaults" entry (with its own confirm
  /// dialog at the call site). Null renders no button.
  final VoidCallback? onReset;

  /// UB2-09: legacy保留区标题灰显（折叠能力与默认展开不变）。
  final bool dimmed;

  @override
  State<SettingsSection> createState() => _SettingsSectionState();
}

class _SettingsSectionState extends State<SettingsSection> {
  late bool _open = widget.expanded ?? !widget.collapsed;

  @override
  void didUpdateWidget(covariant SettingsSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.expanded != null && widget.expanded != _open) {
      _open = widget.expanded!;
    }
  }

  void _toggle() {
    final next = !_open;
    if (widget.expanded != null) {
      widget.onExpandedChanged?.call(next);
      return;
    }
    setState(() => _open = next);
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        InkWell(
          onTap: _toggle,
          child: Padding(
            padding: const EdgeInsets.only(top: 10, bottom: 4),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    widget.title,
                    style: AppForm.sectionTitleStyle(Theme.of(context))
                        .copyWith(
                          color: widget.dimmed ? context.semantics.muted : null,
                        ),
                  ),
                ),
                if (widget.onReset != null && widget.sectionId != null)
                  IconButton(
                    key: ValueKey('settings-reset-${widget.sectionId}'),
                    tooltip: '恢复本组默认',
                    iconSize: 16,
                    visualDensity: VisualDensity.compact,
                    onPressed: widget.onReset,
                    icon: Icon(
                      AppTokens.icon('refresh'),
                      color: context.semantics.muted,
                    ),
                  ),
                Icon(
                  AppTokens.icon(_open ? 'arrowUp' : 'dropdown'),
                  size: 16,
                  color: context.semantics.muted,
                ),
              ],
            ),
          ),
        ),
        const Divider(height: 1),
        if (_open) ...widget.child,
        const SizedBox(height: 8),
      ],
    );
  }
}

/// Upstream note banner for a not-yet-wired platform action.
class SettingsNote extends StatelessWidget {
  const SettingsNote(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      margin: const EdgeInsets.symmetric(vertical: 4),
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(AppTokens.radiusMedium),
      ),
      child: Text(
        text,
        style: const TextStyle(
          fontSize: AppTokens.fontSizeCaption,
          fontStyle: FontStyle.italic,
        ),
      ),
    );
  }
}

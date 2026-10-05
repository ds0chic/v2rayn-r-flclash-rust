import 'package:flutter/material.dart';

/// Small form primitives bound to a mutable settings group map. They keep the
/// option window declarative while the document stays a plain JSON map.

class SettingsCheckbox extends StatelessWidget {
  const SettingsCheckbox({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
  });

  final String label;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: () => onChanged(!value),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 2),
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
            Expanded(child: Text(label, style: const TextStyle(fontSize: 12))),
          ],
        ),
      ),
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
  });

  final String label;
  final String? value;
  final ValueChanged<String?> onChanged;
  final String? hint;
  final double width;

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
      child: Row(
        children: <Widget>[
          SizedBox(
            width: 168,
            child: Text(widget.label, style: const TextStyle(fontSize: 12)),
          ),
          SizedBox(
            width: widget.width,
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
              style: const TextStyle(fontSize: 12),
              onChanged: (text) => widget.onChanged(text.isEmpty ? null : text),
            ),
          ),
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
  });

  final String label;
  final int? value;
  final ValueChanged<int?> onChanged;
  final double width;

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
      child: Row(
        children: <Widget>[
          SizedBox(
            width: 168,
            child: Text(widget.label, style: const TextStyle(fontSize: 12)),
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
              style: const TextStyle(fontSize: 12),
              onChanged: (text) => widget.onChanged(
                text.trim().isEmpty ? null : int.tryParse(text.trim()),
              ),
            ),
          ),
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
  });

  final String label;
  final T? value;
  final List<DropdownMenuItem<T>> items;
  final ValueChanged<T?> onChanged;
  final double width;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        children: <Widget>[
          SizedBox(
            width: 168,
            child: Text(label, style: const TextStyle(fontSize: 12)),
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
        ],
      ),
    );
  }
}

/// A labelled sub-section inside a tab.
class SettingsSection extends StatelessWidget {
  const SettingsSection({super.key, required this.title, required this.child});

  final String title;
  final List<Widget> child;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(top: 10, bottom: 4),
          child: Text(
            title,
            style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w600),
          ),
        ),
        const Divider(height: 1),
        ...child,
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
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        text,
        style: const TextStyle(fontSize: 11, fontStyle: FontStyle.italic),
      ),
    );
  }
}

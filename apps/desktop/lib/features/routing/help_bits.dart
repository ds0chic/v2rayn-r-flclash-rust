import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Shared explanatory bits for the routing-area windows (UI-HELP-BITS).
///
/// Purely additive display layer for the UX3 copy pass (dns_window.dart,
/// routing_windows.dart): small help lines, info tooltips, default-collapsed
/// fold sections and read-only legacy rows. Only U-A tokens are used
/// (`AppForm.helperStyle`, the semantic icon map, semantic colors); no raw
/// numeric text sizes, no direct framework icon references.

Color _muted(BuildContext context) =>
    Theme.of(context).extension<AppSemanticColors>()?.muted ??
    Theme.of(context).colorScheme.onSurfaceVariant;

/// Small explanatory line under a form row.
class HelpText extends StatelessWidget {
  const HelpText(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: 2),
      child: Text(
        text,
        style: AppForm.helperStyle(theme).copyWith(color: _muted(context)),
      ),
    );
  }
}

/// Info affordance showing [message] as a tooltip.
class HelpTip extends StatelessWidget {
  const HelpTip(this.message, {super.key});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: message,
      child: Icon(
        AppTokens.icon('help'),
        size: AppTokens.fontSizeSubtitle,
        color: _muted(context),
      ),
    );
  }
}

/// Default-collapsed section for advanced/expert rows.
///
/// When the folded-away content already holds user values, pass [badge]
/// (e.g. "已设置 2 项") and/or [initiallyExpanded] so effective settings are
/// never silently hidden (U-C1 condition A).
class FoldSection extends StatelessWidget {
  const FoldSection({
    super.key,
    required this.title,
    required this.child,
    this.badge,
    this.initiallyExpanded = false,
  });

  final String title;
  final Widget child;
  final String? badge;
  final bool initiallyExpanded;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ExpansionTile(
      tilePadding: EdgeInsets.zero,
      childrenPadding: const EdgeInsets.only(left: 8),
      initiallyExpanded: initiallyExpanded,
      title: Row(
        children: <Widget>[
          Expanded(child: Text(title, style: AppForm.labelStyle(theme))),
          if (badge != null && badge!.isNotEmpty)
            Text(
              badge!,
              style: AppForm.helperStyle(theme)
                  .copyWith(color: _muted(context)),
            ),
        ],
      ),
      children: <Widget>[child],
    );
  }
}

/// Read-only display of a retained legacy field plus a pending tag.
///
/// Never editable: no TextField/DropdownButton inside. The value passes
/// through saves untouched; semantics marked 待核实 (UX-IMPL-PLAN decision 2).
class LegacyRow extends StatelessWidget {
  const LegacyRow({super.key, required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final style = AppForm.helperStyle(theme).copyWith(color: _muted(context));
    return Padding(
      padding: const EdgeInsets.only(top: 2),
      child: Text(
        '$label：${value.isEmpty ? "（空）" : value}（界面不可改，待核实）',
        style: style,
      ),
    );
  }
}

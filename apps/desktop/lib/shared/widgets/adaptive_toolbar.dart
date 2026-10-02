import 'package:flutter/material.dart';

/// Toolbar that wraps its children onto extra lines at narrow widths so no
/// control is ever clipped. At wide widths it renders as a single compact line.
class AdaptiveToolbar extends StatelessWidget {
  const AdaptiveToolbar({
    super.key,
    required this.children,
    this.padding = const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
    this.spacing = 2,
    this.runSpacing = 2,
  });

  final List<Widget> children;
  final EdgeInsetsGeometry padding;
  final double spacing;
  final double runSpacing;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: padding,
      child: Wrap(
        spacing: spacing,
        runSpacing: runSpacing,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: children,
      ),
    );
  }
}

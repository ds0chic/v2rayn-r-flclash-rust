import 'package:flutter/material.dart';

/// Horizontally scrollable single-line chrome (toolbar / status bar) that keeps
/// every control reachable at narrow widths instead of clipping the trailing
/// buttons.
///
/// A visible scrollbar advertises the overflow; `Scrollbar` is provided with an
/// owned controller so it also works inside `Expanded`/`Row` parents.
class HorizontalToolbar extends StatefulWidget {
  const HorizontalToolbar({
    super.key,
    required this.child,
    this.padding,
    this.showScrollbar = true,
  });

  final Widget child;
  final EdgeInsetsGeometry? padding;
  final bool showScrollbar;

  @override
  State<HorizontalToolbar> createState() => _HorizontalToolbarState();
}

class _HorizontalToolbarState extends State<HorizontalToolbar> {
  final ScrollController _controller = ScrollController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scrollView = SingleChildScrollView(
      controller: _controller,
      scrollDirection: Axis.horizontal,
      padding: widget.padding,
      child: widget.child,
    );
    if (!widget.showScrollbar) return scrollView;
    return Scrollbar(
      controller: _controller,
      thumbVisibility: true,
      trackVisibility: true,
      thickness: 8,
      scrollbarOrientation: ScrollbarOrientation.bottom,
      child: scrollView,
    );
  }
}

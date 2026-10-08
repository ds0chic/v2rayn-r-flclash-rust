import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';

/// Armed-only semantics-tree dump bridge (SP-30 UIA gap workaround).
///
/// The pinned Flutter Windows engine exposes no accessibility bridge (UIA/MSAA
/// see only the FLUTTERVIEW pane), so agents cannot locate widgets via UIA.
/// This hook dumps the Flutter semantics tree — stable identifiers/labels plus
/// global rects — to a JSON file the agent reads to locate elements, then the
/// agent performs real mouse/keyboard input itself.
///
/// Enabled only by `--dart-define=V2RAYN_R_SMOKE_ARMED=true` **plus**
/// `V2RAYN_R_SEMANTICS_DUMP=<absolute file path>` — the same arming pattern as
/// `ImportSynthHook`/`T18Bench`. Unarmed (official) builds ignore the env var
/// entirely.
///
/// Read-only: walks the tree and writes a file. No app state changes, no
/// clicks, no network, no OS calls beyond the file write.
///
/// Root accessor (Flutter 3.47): per [RenderView] in
/// `RendererBinding.instance.renderViews`,
/// `renderView.owner?.semanticsOwner?.rootSemanticsNode`
/// (`SemanticsOwner.rootSemanticsNode` == `_nodes[0]`).
class SemanticsDumpHook {
  static String _env(String key) => Platform.environment[key] ?? '';

  /// Absolute dump-file path from `V2RAYN_R_SEMANTICS_DUMP`.
  static String get outPath => _env('V2RAYN_R_SEMANTICS_DUMP');

  static bool get enabled =>
      const bool.fromEnvironment('V2RAYN_R_SMOKE_ARMED', defaultValue: false) &&
      outPath.isNotEmpty &&
      File(outPath).isAbsolute;

  static SemanticsHandle? _handle;
  static Timer? _timer;

  /// Force semantics on, keep the handle alive, and start the periodic dump.
  /// Safe to call once from `main()` after binding init; no-op when disabled.
  static void start() {
    if (!enabled || _timer != null) return;
    _handle ??= SemanticsBinding.instance.ensureSemantics();
    _dumpOnce();
    _timer = Timer.periodic(
      const Duration(milliseconds: 400),
      (_) => _dumpOnce(),
    );
  }

  static void _dumpOnce() {
    try {
      final binding = RendererBinding.instance;
      double dpr = 1.0;
      double winW = 0.0;
      double winH = 0.0;
      final views = binding.platformDispatcher.views;
      if (views.isNotEmpty) {
        dpr = views.first.devicePixelRatio;
        winW = views.first.physicalSize.width / dpr;
        winH = views.first.physicalSize.height / dpr;
      }
      final roots = <Map<String, dynamic>>[];
      var count = 0;
      var index = 0;
      for (final view in binding.renderViews) {
        final root = view.owner?.semanticsOwner?.rootSemanticsNode;
        Map<String, dynamic>? tree;
        if (root != null) {
          tree = _nodeJson(root, Matrix4.identity());
          count += _count(tree);
        }
        roots.add(<String, dynamic>{'index': index, 'tree': tree});
        index++;
      }
      final payload = <String, dynamic>{
        'tool': 'v2rayn-r semantics_dump',
        'status': count > 0 ? 'ok' : 'waiting',
        'pid': pid,
        'devicePixelRatio': dpr,
        'windowSize': <double>[winW, winH],
        'nodeCount': count,
        'roots': roots,
      };
      _writeAtomic(File(outPath), jsonEncode(payload));
    } catch (error) {
      stderr.writeln('[semantics-dump] skipped: $error');
    }
  }

  static Map<String, dynamic> _nodeJson(
    SemanticsNode node,
    Matrix4 parentToGlobal,
  ) {
    final Matrix4 toGlobal = parentToGlobal.clone();
    final Matrix4? local = node.transform;
    if (local != null) toGlobal.multiply(local);
    final SemanticsData data = node.getSemanticsData();
    final actions = <String>[];
    for (final ui.SemanticsAction action in ui.SemanticsAction.values) {
      if ((data.actions & action.index) != 0) actions.add(action.name);
    }
    final flags = <String, Object>{};
    final ui.SemanticsFlags f = data.flagsCollection;
    if (f.isSelected != ui.Tristate.none) {
      flags['selected'] = f.isSelected == ui.Tristate.isTrue;
    }
    if (f.isChecked != ui.CheckedState.none) {
      flags['checked'] = f.isChecked == ui.CheckedState.isTrue
          ? true
          : f.isChecked == ui.CheckedState.isFalse
          ? false
          : 'mixed';
    }
    if (f.isEnabled != ui.Tristate.none) {
      flags['enabled'] = f.isEnabled == ui.Tristate.isTrue;
    }
    if (f.isFocused != ui.Tristate.none) {
      flags['focused'] = f.isFocused == ui.Tristate.isTrue;
    }
    final children = <Map<String, dynamic>>[];
    node.visitChildren((SemanticsNode child) {
      children.add(_nodeJson(child, toGlobal));
      return true;
    });
    return <String, dynamic>{
      'id': node.id,
      'identifier': data.identifier,
      'label': data.label,
      'value': data.value,
      'rect': _globalRect(node.rect, toGlobal),
      'actions': actions,
      'flags': flags,
      'children': children,
    };
  }

  /// Node rect (own coordinate space) mapped to global logical pixels
  /// [x, y, w, h] through the accumulated 2D transform (column-major storage:
  /// x' = m0*x + m4*y + m12, y' = m1*x + m5*y + m13).
  static List<double> _globalRect(ui.Rect rect, Matrix4 toGlobal) {
    final s = toGlobal.storage;
    double minX = double.infinity;
    double minY = double.infinity;
    double maxX = double.negativeInfinity;
    double maxY = double.negativeInfinity;
    for (final corner in <List<double>>[
      [rect.left, rect.top],
      [rect.right, rect.top],
      [rect.left, rect.bottom],
      [rect.right, rect.bottom],
    ]) {
      final x = s[0] * corner[0] + s[4] * corner[1] + s[12];
      final y = s[1] * corner[0] + s[5] * corner[1] + s[13];
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
    }
    if (minX.isInfinite) return <double>[0, 0, 0, 0];
    return <double>[minX, minY, maxX - minX, maxY - minY];
  }

  static int _count(Map<String, dynamic> tree) {
    var n = 1;
    for (final child in (tree['children'] as List)) {
      n += _count(child as Map<String, dynamic>);
    }
    return n;
  }

  /// Atomic write: temp file in the same directory, then rename over target.
  static void _writeAtomic(File target, String text) {
    target.parent.createSync(recursive: true);
    final tmp = File('${target.path}.tmp.$pid');
    tmp.writeAsStringSync(text);
    try {
      tmp.renameSync(target.path);
    } on FileSystemException {
      // Windows: rename over an existing file can fail; replace explicitly.
      if (target.existsSync()) target.deleteSync();
      tmp.renameSync(target.path);
    }
  }
}

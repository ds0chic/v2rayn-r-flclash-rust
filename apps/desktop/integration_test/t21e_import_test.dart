// T21-E real-device test: drives the packaged Windows app through the real
// top menu, the real clipboard and the real Rust bridge + SQLite, then
// verifies the subscription-URL guidance path against a local loopback server.
//
// Run with a clean data directory:
//   $env:V2RAYN_R_DATA_DIR = <temp>
//   flutter test integration_test/t21e_import_test.dart -d windows
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/main.dart' as app;

const String _shareLinks =
    'vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one\n'
    'vmess://eyJ2IjoiMiIsInBzIjoidHdvIiwiYWRkIjoiYi5leGFtcGxlIiwicG9ydCI6IjQ0MyIsImlkIjoiMjIyMjIyMjItMjIyMi0yMjIyLTIyMjItMjIyMjIyMjIyMjIyIiwiYWlkIjoiMCIsIm5ldCI6IndzIiwidHlwZSI6Im5vbmUiLCJob3N0IjoiIiwicGF0aCI6Ii8iLCJ0bHMiOiIifQ==\n'
    'trojan://password@c.example:443#three\n'
    'ss://YWVzLTI1Ni1nY206c2VjcmV0@d.example:8388#four';

Future<void> _pumpUntil(
  WidgetTester tester,
  bool Function() done, {
  Duration timeout = const Duration(seconds: 30),
  String? reason,
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    await tester.pump(const Duration(milliseconds: 200));
    if (done()) return;
  }
  throw TestFailure(reason ?? 'condition not met within $timeout');
}

Future<void> _openMenu(WidgetTester tester, String group, String item) async {
  await tester.tap(find.byKey(ValueKey('menu-$group')));
  await tester.pump(const Duration(milliseconds: 400));
  await tester.tap(find.byKey(ValueKey('menu-item-$item')));
  await tester.pump(const Duration(milliseconds: 400));
}

Future<HttpServer> _bindLoopback() async {
  for (var port = 11821; port <= 11831; port++) {
    try {
      return await HttpServer.bind(InternetAddress.loopbackIPv4, port);
    } on SocketException {
      continue;
    }
  }
  throw StateError('no free loopback port in 11821..11831');
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('clipboard import and subscription guidance survive the real '
      'menu + bridge', (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    expect(dataDir, isNotNull, reason: 'V2RAYN_R_DATA_DIR must be set');

    // Local loopback subscription server (port >= 11808, never 10808).
    final server = await _bindLoopback();
    var hit = false;
    final subBody = base64Encode(
      utf8.encode(
        'vless://33333333-3333-3333-3333-333333333333@subnode.example:443?encryption=none#sub-one\n'
        'vless://44444444-4444-4444-4444-444444444444@subnode2.example:443?encryption=none#sub-two',
      ),
    );
    server.listen((request) {
      hit = true;
      request.response.headers.contentType = ContentType.text;
      request.response.write(subBody);
      request.response.close();
    });
    addTearDown(() => server.close(force: true));
    final subUrl = 'http://127.0.0.1:${server.port}/sub';

    // 1) Share links through the real clipboard + real menu + real bridge.
    await Clipboard.setData(const ClipboardData(text: _shareLinks));
    await app.main();
    await _pumpUntil(
      tester,
      () => find.byKey(const ValueKey('menu-配置项')).evaluate().isNotEmpty,
    );

    await _openMenu(tester, '配置项', '从剪贴板导入分享链接');
    await _pumpUntil(
      tester,
      () => find.textContaining('已从剪贴板导入').evaluate().isNotEmpty,
      reason: 'no import status message appeared',
    );
    await _pumpUntil(
      tester,
      () => find.text('one').evaluate().isNotEmpty,
      reason: 'imported node "one" never appeared in the real table',
    );
    expect(find.text('two').evaluate(), isNotEmpty);
    expect(find.text('three').evaluate(), isNotEmpty);
    expect(find.text('four').evaluate(), isNotEmpty);

    // 2) A subscription URL on the clipboard is offered as a subscription.
    await Clipboard.setData(ClipboardData(text: subUrl));
    await _openMenu(tester, '配置项', '从剪贴板导入分享链接');
    await _pumpUntil(
      tester,
      () => find
          .byKey(const ValueKey('sub-url-import-dialog'))
          .evaluate()
          .isNotEmpty,
      reason: 'subscription URL guidance dialog did not appear',
    );
    await tester.tap(find.byKey(const ValueKey('sub-url-import-add')));
    await tester.pump(const Duration(milliseconds: 400));

    await _pumpUntil(
      tester,
      () => hit && find.text('sub-one').evaluate().isNotEmpty,
      timeout: const Duration(seconds: 40),
      reason: 'subscription download/import never reached the table',
    );
    expect(find.text('sub-two').evaluate(), isNotEmpty);
  });
}

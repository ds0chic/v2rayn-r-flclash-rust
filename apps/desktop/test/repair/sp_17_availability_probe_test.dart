import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';

/// SP-17 red contract: the original availability test (upstream
/// `TestServerAvailability`, bound to PreviewMouseDown on the running texts)
/// reports real loopback feedback for the actual applied endpoint: reachable
/// with timing, or an honest failure. It never probes the reserved 10808
/// port and never fabricates success without an applied endpoint.
void main() {
  test('loopback probe reports real reachability with timing', () async {
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(server.close);
    expect(server.port, isNot(10808));

    final result = await probeLoopbackEndpoint(server.port);

    expect(result.reachable, isTrue);
    expect(result.endpoint, contains('${server.port}'));
    expect(result.elapsedMs, greaterThanOrEqualTo(0));
  });

  test('loopback probe reports a closed port honestly', () async {
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    final port = server.port;
    await server.close();

    final result = await probeLoopbackEndpoint(port);

    expect(result.reachable, isFalse);
    expect(result.detail, isNotNull);
  });

  test('loopback probe refuses the reserved 10808 port', () async {
    final result = await probeLoopbackEndpoint(10808);

    expect(result.reachable, isFalse);
    expect(result.detail, contains('10808'));
  });
}

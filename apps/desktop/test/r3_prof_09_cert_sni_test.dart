// R3-PROF-09: certificate fetch must connect to the node's Address:Port and
// send the selected SNI document domain, without resolving that domain. The
// fixture is a plain loopback listener that captures the TLS ClientHello; it
// never completes a handshake, so only the SNI bytes are asserted. Ports are
// probed from 11808 and never touch the user's 10808.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';

Future<ServerSocket> _bindLoopback() async {
  for (var port = 11808; port <= 11940; port++) {
    try {
      return await ServerSocket.bind(InternetAddress.loopbackIPv4, port);
    } on SocketException {
      continue;
    }
  }
  throw StateError('no free test port >= 11808');
}

void main() {
  test(
    'cert fetch connects to the Address and sends the SNI without DNS',
    () async {
      final server = await _bindLoopback();
      final port = server.port;
      final received = Completer<List<int>>();
      final buffer = <int>[];

      final sub = server.listen((socket) {
        socket.listen(
          (data) {
            buffer.addAll(data);
            if (!received.isCompleted &&
                (buffer.length >= 512 ||
                    latin1
                        .decode(buffer, allowInvalid: true)
                        .contains('www.example.test'))) {
              received.complete(List<int>.of(buffer));
              socket.destroy();
            }
          },
          onError: (_) {},
          onDone: () {
            if (!received.isCompleted) received.complete(List<int>.of(buffer));
          },
          cancelOnError: true,
        );
      });
      addTearDown(() async {
        await sub.cancel();
        await server.close();
      });

      // Address is the loopback IP; SNI is a reserved documentation-only domain
      // that must never be resolved. The handshake cannot complete (the fixture
      // is not a TLS server), so the failure is deliberately ignored.
      await fetchPeerCertPem(
        host: '127.0.0.1',
        port: port,
        serverName: 'www.example.test',
        timeout: const Duration(seconds: 5),
      ).then<void>((_) {}, onError: (_) {});

      final hello = await received.future.timeout(const Duration(seconds: 5));
      expect(
        latin1.decode(hello, allowInvalid: true).contains('www.example.test'),
        isTrue,
        reason: 'ClientHello must carry the documentation SNI, not the IP',
      );
    },
  );
}

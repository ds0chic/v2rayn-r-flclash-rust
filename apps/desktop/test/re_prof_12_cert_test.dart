import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';

/// RE-PROF-12: certificate helper boundaries. Pure functions only; no socket,
/// no real certificate or node is touched.
void main() {
  group('selectCertFetchServerName', () {
    test('prefers SNI, then transport host, then address', () {
      expect(
        selectCertFetchServerName(
          sni: 'sni.example.invalid',
          transportHost: 'host.example.invalid',
          address: '192.0.2.1',
        ),
        'sni.example.invalid',
      );
      expect(
        selectCertFetchServerName(
          sni: '   ',
          transportHost: 'host.example.invalid',
          address: '192.0.2.1',
        ),
        'host.example.invalid',
      );
      expect(
        selectCertFetchServerName(
          sni: null,
          transportHost: '',
          address: '192.0.2.1',
        ),
        '192.0.2.1',
      );
    });
  });

  group('certSha256Thumbprint', () {
    test('hashes a valid PEM body', () {
      final sha = certSha256Thumbprint(
        '-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----',
      );
      expect(sha, matches(RegExp(r'^[0-9a-f]{64}$')));
    });

    test('returns null for invalid base64 without throwing', () {
      expect(
        certSha256Thumbprint(
          '-----BEGIN CERTIFICATE-----\nnot base64 !!!\n'
          '-----END CERTIFICATE-----',
        ),
        isNull,
      );
      expect(
        () => certSha256Thumbprint(
          '-----BEGIN CERTIFICATE-----\n%%%%\n-----END CERTIFICATE-----',
        ),
        returnsNormally,
      );
    });

    test('returns null when no PEM block is present', () {
      expect(certSha256Thumbprint('not a certificate'), isNull);
    });
  });

  group('certShaFromChain', () {
    test('joins valid blocks and refuses a malformed block', () {
      final ok = certShaFromChain(
        '-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----',
      );
      expect(ok, matches(RegExp(r'^[0-9a-f]{64}$')));

      final bad = certShaFromChain(
        '-----BEGIN CERTIFICATE-----\n%%%\n-----END CERTIFICATE-----',
      );
      expect(bad, isNull);
    });
  });

  group('PeerCertChainResult', () {
    test('reports leaf-only so the UI never claims a full chain', () {
      final result = peerCertChainResult(
        '-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----',
      );
      expect(result, isNotNull);
      expect(result!.leafOnly, isTrue);
      expect(peerCertChainResult(null), isNull);
    });
  });
}

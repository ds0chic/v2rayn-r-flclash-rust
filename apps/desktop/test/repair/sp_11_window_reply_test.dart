// SP-11: independent-window save replies must confirm the real result and
// never wait forever (IMPLEMENTATION_PLAN §3.4, CP-09).
//
// Correct contract:
// - MethodChannel transport ACK (saveDraft returns) is NOT the business
//   outcome; the outcome arrives via saveOutcome/reportOutcome.
// - A missing outcome settles within a bounded wait as PendingConfirmation;
//   expiry must query (reconciler seam), never auto-replay a non-idempotent
//   save, and never hang.
// - Late / duplicate / after-close replies are filtered by
//   (windowGeneration, requestId); a new window never accepts old replies.
// - A main-side save exception still produces a structured failed outcome.
// - The normal ok path and legacy replies without an envelope keep working.
//
// Fault injection only: synthetic snapshots/drafts, mock channel that drops,
// delays, or duplicates replies. No real user data, no host side effects.
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';

const _settingsReady = '{"GuiItem":{"AutoRun":false}}';
const _routingReady =
    '{"schemes":[],"domainStrategy":"AsIs",'
    '"domainStrategy4Singbox":"","outboundTags":[]}';

void main() {
  group('SP-11 window reply loss', () {
    testWidgets('settings lost saveOutcome settles instead of hanging', (
      tester,
    ) async {
      const channel = MethodChannel(kOptionWindowChannel);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _settingsReady;
        if (call.method == 'saveDraft') {
          return null; // Transport ACK; the outcome reply is lost.
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeSettingsEditorHost();
      await host.loadSnapshot();
      var settled = false;
      SettingsEditorOutcome? outcome;
      host
          .save(<String, dynamic>{
            'GuiItem': <String, dynamic>{'AutoRun': false},
          })
          .then((value) {
            settled = true;
            outcome = value;
          });
      await tester.pump(const Duration(seconds: 12));
      expect(
        settled,
        isTrue,
        reason:
            'transport ACK is not the save result; '
            'a lost saveOutcome must settle as PendingConfirmation',
      );
      expect(outcome!.ok, isFalse);
      expect(outcome!.pendingConfirmation, isTrue);
    });

    testWidgets('routing lost commit outcome settles instead of hanging', (
      tester,
    ) async {
      const channel = MethodChannel('v2rayn/routing_window');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _routingReady;
        if (call.method == 'saveDraft') {
          return null; // Transport ACK; the outcome reply is lost.
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeRoutingEditorHost();
      await host.loadSnapshot();
      var settled = false;
      RoutingEditorOutcome? outcome;
      host
          .commit(
            '{"kind":"strategy","domainStrategy":"AsIs",'
            '"domainStrategySbox":""}',
          )
          .then((value) {
            settled = true;
            outcome = value;
          });
      await tester.pump(const Duration(seconds: 12));
      expect(
        settled,
        isTrue,
        reason:
            'a lost routing outcome must settle as PendingConfirmation '
            'instead of leaving the window busy forever',
      );
      expect(outcome!.ok, isFalse);
      expect(outcome!.pendingConfirmation, isTrue);
    });

    testWidgets('close settles pending saves instead of stranding them', (
      tester,
    ) async {
      const channel = MethodChannel(kOptionWindowChannel);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _settingsReady;
        if (call.method == 'saveDraft') {
          return null; // Transport ACK; the outcome reply never comes.
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeSettingsEditorHost();
      await host.loadSnapshot();
      var settled = false;
      host
          .save(<String, dynamic>{
            'GuiItem': <String, dynamic>{'AutoRun': false},
          })
          .then((_) {
            settled = true;
          });
      await tester.pump(const Duration(milliseconds: 100));
      await host.close();
      await tester.pump();
      expect(
        settled,
        isTrue,
        reason:
            'close must settle pending saves; '
            'late replies after close must be filtered',
      );
    });

    // NOTE: driving the main-side handler through `handlePlatformMessage`
    // with the mock removed is intentionally not tested: without a mock,
    // the fixed handler's `reportOutcome` invoke has no peer and never
    // completes in the test binding, so such a test would time out instead
    // of asserting anything. The structured exception outcome is covered
    // precisely by `debugDispatchApplyDraft` below.
    testWidgets('main-side exception sends a structured failed outcome', (
      tester,
    ) async {
      const channel = MethodChannel(kOptionWindowChannel);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      final reported = <Map<String, dynamic>>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'open') return true;
        if (call.method == 'reportOutcome') {
          reported.add((call.arguments as Map).cast<String, dynamic>());
          return null;
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
      await OptionWindowHost.instance.open(
        snapshot: const <String, dynamic>{},
        onSave: (_) async => throw StateError('synthetic save failure'),
      );
      await OptionWindowHost.instance.debugDispatchApplyDraft(<String, dynamic>{
        'id': 7,
        'requestId': 'req-1-7',
        'windowGeneration': 1,
        'draft': '{}',
      });
      expect(reported, hasLength(1));
      expect(reported.single['ok'], isFalse);
      expect(reported.single['status'], 'failed');
      expect(reported.single['requestId'], 'req-1-7');
      expect(reported.single['windowGeneration'], 1);
    });

    testWidgets('settings ok reply completes with the real result', (
      tester,
    ) async {
      const channel = MethodChannel(kOptionWindowChannel);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      final sent = <Map<String, dynamic>>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _settingsReady;
        if (call.method == 'saveDraft') {
          sent.add((call.arguments as Map).cast<String, dynamic>());
          return null;
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeSettingsEditorHost(
        outcomeTimeout: const Duration(milliseconds: 200),
      );
      await host.loadSnapshot();
      final pending = host.save(<String, dynamic>{
        'GuiItem': <String, dynamic>{'AutoRun': false},
      });
      await tester.pump();
      expect(sent, hasLength(1));
      expect(sent.single['requestId'], isNotEmpty);
      // The envelope must travel with the request (additive: the C++ runner
      // relays the map opaquely).
      await host.debugInjectReply(<String, dynamic>{
        ...sent.single,
        'ok': true,
        'status': 'ok',
      });
      final outcome = await pending;
      expect(outcome.ok, isTrue);
      expect(outcome.pendingConfirmation, isFalse);
      expect(host.debugPendingCount, 0);
    });

    testWidgets('settings duplicate reply is ignored', (tester) async {
      const channel = MethodChannel(kOptionWindowChannel);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      final sent = <Map<String, dynamic>>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _settingsReady;
        if (call.method == 'saveDraft') {
          sent.add((call.arguments as Map).cast<String, dynamic>());
          return null;
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeSettingsEditorHost(
        outcomeTimeout: const Duration(milliseconds: 200),
      );
      await host.loadSnapshot();
      final pending = host.save(<String, dynamic>{
        'GuiItem': <String, dynamic>{'AutoRun': false},
      });
      await tester.pump();
      await host.debugInjectReply(<String, dynamic>{
        ...sent.single,
        'ok': true,
        'status': 'ok',
      });
      // Duplicate redelivery of the same outcome must not change anything.
      await host.debugInjectReply(<String, dynamic>{
        ...sent.single,
        'ok': false,
        'status': 'failed',
        'message': 'stale duplicate',
      });
      final outcome = await pending;
      expect(outcome.ok, isTrue);
      expect(host.debugPendingCount, 0);
    });

    testWidgets(
      'settings late reply after timeout is dropped without resending',
      (tester) async {
        const channel = MethodChannel(kOptionWindowChannel);
        final messenger =
            TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
        var saveDrafts = 0;
        final sent = <Map<String, dynamic>>[];
        messenger.setMockMethodCallHandler(channel, (call) async {
          if (call.method == 'ready') return _settingsReady;
          if (call.method == 'saveDraft') {
            saveDrafts++;
            sent.add((call.arguments as Map).cast<String, dynamic>());
            return null;
          }
          return null;
        });
        addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

        final host = NativeSettingsEditorHost(
          outcomeTimeout: const Duration(milliseconds: 200),
        );
        await host.loadSnapshot();
        final pending = host.save(<String, dynamic>{
          'GuiItem': <String, dynamic>{'AutoRun': false},
        });
        await tester.pump(const Duration(seconds: 1));
        final outcome = await pending;
        expect(outcome.pendingConfirmation, isTrue);
        expect(outcome.ok, isFalse);
        // The late reply must be dropped, and expiry must never resend.
        await host.debugInjectReply(<String, dynamic>{
          ...sent.single,
          'ok': true,
          'status': 'ok',
        });
        await tester.pump(const Duration(seconds: 1));
        expect(saveDrafts, 1);
        expect(host.debugPendingCount, 0);
      },
    );

    testWidgets('settings close rotates the generation; old replies dropped', (
      tester,
    ) async {
      const channel = MethodChannel(kOptionWindowChannel);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      final sent = <Map<String, dynamic>>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _settingsReady;
        if (call.method == 'saveDraft') {
          sent.add((call.arguments as Map).cast<String, dynamic>());
          return null;
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeSettingsEditorHost(
        outcomeTimeout: const Duration(milliseconds: 200),
      );
      await host.loadSnapshot();
      final first = host.save(<String, dynamic>{
        'GuiItem': <String, dynamic>{'AutoRun': false},
      });
      await tester.pump();
      final generation = host.debugGeneration;
      await host.close();
      final closed = await first;
      expect(closed.ok, isFalse);
      expect(host.debugGeneration, generation + 1);
      // The closing window's late reply carries the old generation: dropped.
      await host.debugInjectReply(<String, dynamic>{
        ...sent.single,
        'ok': true,
        'status': 'ok',
      });
      expect(host.debugPendingCount, 0);
      // A new request on the rotated generation works normally.
      final second = host.save(<String, dynamic>{
        'GuiItem': <String, dynamic>{'AutoRun': true},
      });
      await tester.pump();
      await host.debugInjectReply(<String, dynamic>{
        ...sent.last,
        'ok': true,
        'status': 'ok',
      });
      expect((await second).ok, isTrue);
    });

    testWidgets('settings legacy reply without envelope still completes', (
      tester,
    ) async {
      const channel = MethodChannel(kOptionWindowChannel);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _settingsReady;
        if (call.method == 'saveDraft') return null;
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeSettingsEditorHost(
        outcomeTimeout: const Duration(milliseconds: 200),
      );
      await host.loadSnapshot();
      final pending = host.save(<String, dynamic>{
        'GuiItem': <String, dynamic>{'AutoRun': false},
      });
      await tester.pump();
      // An old main engine echoes only the numeric id: still accepted.
      await host.debugInjectReply(<String, dynamic>{'id': 1, 'ok': true});
      expect((await pending).ok, isTrue);
    });

    testWidgets('settings timeout queries instead of replaying', (
      tester,
    ) async {
      const channel = MethodChannel(kOptionWindowChannel);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      var saveDrafts = 0;
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _settingsReady;
        if (call.method == 'saveDraft') {
          saveDrafts++;
          return null;
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final queried = <WindowPendingQuery>[];
      final reconciled = <SettingsEditorOutcome>[];
      final host = NativeSettingsEditorHost(
        outcomeTimeout: const Duration(milliseconds: 200),
        reconciler: (query) async {
          queried.add(query);
          return null; // Unknown: stays PendingConfirmation.
        },
        onReconciled: reconciled.add,
      );
      await host.loadSnapshot();
      final outcome = await (() async {
        final pending = host.save(<String, dynamic>{
          'GuiItem': <String, dynamic>{'AutoRun': false},
        });
        await tester.pump(const Duration(seconds: 1));
        return pending;
      })();
      expect(outcome.pendingConfirmation, isTrue);
      await tester.pump();
      expect(queried, hasLength(1));
      expect(queried.single.mutationId, isNotEmpty);
      expect(saveDrafts, 1);
    });

    testWidgets('routing ok reply completes; duplicate ignored', (
      tester,
    ) async {
      const channel = MethodChannel('v2rayn/routing_window');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      final sent = <Map<String, dynamic>>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _routingReady;
        if (call.method == 'saveDraft') {
          sent.add((call.arguments as Map).cast<String, dynamic>());
          return null;
        }
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeRoutingEditorHost(
        outcomeTimeout: const Duration(milliseconds: 200),
      );
      await host.loadSnapshot();
      final pending = host.save(
        const RoutingDraft(
          schemes: [],
          domainStrategy: 'AsIs',
          domainStrategySbox: '',
        ),
      );
      await tester.pump();
      expect(sent, hasLength(1));
      await host.debugInjectReply(<String, dynamic>{
        ...sent.single,
        'ok': true,
        'status': 'ok',
      });
      await host.debugInjectReply(<String, dynamic>{
        ...sent.single,
        'ok': false,
        'status': 'failed',
        'message': 'stale duplicate',
      });
      final outcome = await pending;
      expect(outcome.ok, isTrue);
      expect(host.debugPendingCount, 0);
    });

    testWidgets('routing close rotates the generation; old replies dropped', (
      tester,
    ) async {
      const channel = MethodChannel('v2rayn/routing_window');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'ready') return _routingReady;
        if (call.method == 'saveDraft') return null;
        return null;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));

      final host = NativeRoutingEditorHost(
        outcomeTimeout: const Duration(milliseconds: 200),
      );
      await host.loadSnapshot();
      final first = host.commit('{"kind":"strategy"}');
      await tester.pump(const Duration(milliseconds: 50));
      final generation = host.debugGeneration;
      await host.close();
      expect((await first).ok, isFalse);
      expect(host.debugGeneration, generation + 1);
      expect(host.debugPendingCount, 0);
    });
  });
}

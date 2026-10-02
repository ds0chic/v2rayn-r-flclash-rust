import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/profiles_harness.dart';
import 'support/subs_harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('import helpers', () {
    test('extractSubscriptionUrls keeps only http(s) lines', () {
      const text =
          'https://sub.example/a\n'
          'vless://x@a.example:443#one\n'
          '  http://127.0.0.1:11808/sub?token=1  \n'
          '\n';
      expect(extractSubscriptionUrls(text), <String>[
        'https://sub.example/a',
        'http://127.0.0.1:11808/sub?token=1',
      ]);
      expect(looksLikeSubscriptionPayload(text), isFalse);
      expect(
        looksLikeSubscriptionPayload('https://sub.example/a\nhttps://b/c'),
        isTrue,
      );
    });

    test('describeImportFailure is actionable, never generic', () {
      final result = c.ImportResult(
        ok: false,
        imported: 0,
        profiles: const <c.ProfileDto>[],
        errors: const <c.ParseIssueDto>[
          c.ParseIssueDto(
            code: 'E_SUB_UNSUPPORTED',
            message: 'no profile could be parsed from the content',
            itemIndex: 1,
          ),
        ],
      );
      final message = describeImportFailure(result);
      expect(message, contains('第 2 项'));
      expect(message, contains('未识别'));
      expect(message, isNot('导入失败'));
    });
  });

  late List<String> clipboard;
  late Map<String, dynamic> clipboardData;

  setUp(() {
    clipboard = <String>[];
    clipboardData = <String, dynamic>{};
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          switch (call.method) {
            case 'Clipboard.getData':
              return clipboardData;
            case 'Clipboard.setData':
              clipboard.add((call.arguments as Map)['text'] as String);
              return null;
            default:
              return null;
          }
        });
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null);
  });

  Future<(BuildContext, WidgetRef, ProviderContainer)> pumpProbe(
    WidgetTester tester,
  ) async {
    final container = makeContainer(rows: 5);
    addTearDown(container.dispose);
    (BuildContext, WidgetRef)? captured;
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: RefProbe(onRef: (context, ref) => captured = (context, ref)),
          ),
        ),
      ),
    );
    await tester.pump();
    final (context, ref) = captured!;
    return (context, ref, container);
  }

  testWidgets('share links from clipboard persist and appear in the table', (
    tester,
  ) async {
    clipboardData = <String, dynamic>{
      'text':
          'vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one\n'
          'trojan://pw@b.example:443#two',
    };
    final (context, ref, container) = await pumpProbe(tester);
    final before = container.read(profilesControllerProvider).profiles.length;

    await importFromClipboard(context, ref);
    await tester.pump();

    final after = container.read(profilesControllerProvider).profiles;
    expect(after.length - before, 2, reason: 'imported rows must be visible');
    final message = container.read(uiShellControllerProvider).message;
    expect(message, contains('导入 2 个节点'));
  });

  testWidgets('subscription URL offers 作为订阅添加 and adds it', (tester) async {
    clipboardData = <String, dynamic>{'text': 'http://127.0.0.1:11809/sub'};
    final (context, ref, container) = await pumpProbe(tester);

    // The dialog future only completes once the user answers, so trigger
    // without awaiting and drive the UI through pumps.
    final pending = importFromClipboard(context, ref);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    expect(find.byKey(const ValueKey('sub-url-import-dialog')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('sub-url-import-add')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    await pending;

    final subs = container.read(subsControllerProvider).items;
    expect(subs.any((s) => s.url == 'http://127.0.0.1:11809/sub'), isTrue);
    expect(container.read(uiShellControllerProvider).message, contains('订阅'));
  });

  testWidgets('unrecognized clipboard shows a classified failure', (
    tester,
  ) async {
    clipboardData = <String, dynamic>{'text': '这不是任何链接'};
    final (context, ref, container) = await pumpProbe(tester);

    await importFromClipboard(context, ref);
    await tester.pump();

    final message = container.read(uiShellControllerProvider).message;
    expect(message, isNotNull);
    expect(message, contains('导入失败'));
    expect(message, contains('未识别'));
  });
}

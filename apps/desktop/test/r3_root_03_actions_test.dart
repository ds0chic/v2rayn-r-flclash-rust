import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/dns.dart' as dns;
import 'package:v2rayn_desktop/features/settings/settings_actions.dart';

/// R3-ROOT-03: the desktop menu actions (elevated restart, UWP loopback,
/// region preset, core website) are wired through pure command builders plus an
/// injectable launcher, so no host process is started in tests.
void main() {
  test(
    'admin relaunch request carries the runner rebootas marker and runas',
    () {
      final request = buildAdminRelaunchRequest(r'C:\Apps\v2rayn_desktop.exe');
      expect(request.exePath, r'C:\Apps\v2rayn_desktop.exe');
      expect(request.arguments, <String>['rebootas']);
      expect(request.verb, 'runas');
      expect(kRebootAsArgument, 'rebootas');
    },
  );

  test('elevation argv invokes PowerShell Start-Process -Verb RunAs', () {
    final args = buildElevationPowerShellArgs(
      buildAdminRelaunchRequest(r'C:\Apps\v2rayn_desktop.exe'),
    );
    expect(args.first, '-NoProfile');
    expect(args, contains('-Command'));
    final command = args.last;
    expect(command, contains('Start-Process'));
    expect(command, contains('-Verb runas'));
    expect(command, contains("'rebootas'"));
    expect(command, contains(r"'C:\Apps\v2rayn_desktop.exe'"));
  });

  test(
    'relaunchAsAdmin reports hand-off, cancel and failure honestly',
    () async {
      final calls = <List<String>>[];
      Future<ProcessResult> ok(String program, List<String> args) async {
        calls.add(<String>[program, ...args]);
        return ProcessResult(1, 0, '', '');
      }

      expect(
        await relaunchAsAdmin(exePath: r'C:\x\app.exe', launcher: ok),
        isTrue,
      );
      expect(calls.single.first, 'powershell.exe');

      Future<ProcessResult> denied(String p, List<String> a) async =>
          ProcessResult(1, 1, '', 'cancelled');
      expect(
        await relaunchAsAdmin(exePath: r'C:\x\app.exe', launcher: denied),
        isFalse,
      );

      Future<ProcessResult> boom(String p, List<String> a) async =>
          throw const ProcessException('powershell.exe', <String>[], 'missing');
      expect(
        await relaunchAsAdmin(exePath: r'C:\x\app.exe', launcher: boom),
        isFalse,
      );
      expect(await relaunchAsAdmin(exePath: '   ', launcher: ok), isFalse);
    },
  );

  test('UWP loopback command is CheckNetIsolation and reversible', () {
    final add = buildLoopbackExemptionCommand(
      appContainer: 'Contoso.App_8wekyb',
    );
    expect(add.program, 'CheckNetIsolation.exe');
    expect(add.arguments, <String>[
      'LoopbackExempt',
      '-a',
      '-n=Contoso.App_8wekyb',
    ]);
    expect(add.reversible, isTrue);
    expect(add.summary, contains('-d -n=Contoso.App_8wekyb'));

    final remove = buildLoopbackExemptionCommand(
      appContainer: 'Contoso.App_8wekyb',
      mode: LoopbackExemptionMode.remove,
    );
    expect(remove.arguments, <String>[
      'LoopbackExempt',
      '-d',
      '-n=Contoso.App_8wekyb',
    ]);

    final template = buildLoopbackExemptionCommand();
    expect(template.arguments, <String>[
      'LoopbackExempt',
      '-a',
      '-n=<PackageFamilyName>',
    ]);

    final bundled = buildLoopbackExemptionCommand(
      bundledToolPath: 'EnableLoopback.exe',
    );
    expect(bundled.program, 'EnableLoopback.exe');
    expect(bundled.arguments, isEmpty);
  });

  test('core website URLs match upstream CoreUrls and drop /releases', () {
    expect(coreWebsiteUrl('Xray'), 'https://github.com/XTLS/Xray-core');
    expect(coreWebsiteUrl('Sing box'), 'https://github.com/SagerNet/sing-box');
    expect(coreWebsiteUrl('Mihomo'), 'https://github.com/MetaCubeX/mihomo');
    // v2fly and hysteria are excluded from the upstream help menu.
    expect(coreWebsiteUrl('V2fly'), '');
    expect(coreWebsiteUrl('Hysteria'), '');
    expect(buildOpenUrlArgs('https://github.com/XTLS/Xray-core'), <String>[
      '/c',
      'start',
      '',
      'https://github.com/XTLS/Xray-core',
    ]);
  });

  test('openCoreWebsite launches cmd start with the resolved URL', () async {
    final calls = <List<String>>[];
    Future<ProcessResult> recording(String program, List<String> args) async {
      calls.add(<String>[program, ...args]);
      return ProcessResult(1, 0, '', '');
    }

    expect(
      await openCoreWebsite(coreName: 'Xray', launcher: recording),
      isTrue,
    );
    expect(calls.single, <String>[
      'cmd',
      '/c',
      'start',
      '',
      'https://github.com/XTLS/Xray-core',
    ]);
  });

  test(
    'region preset message reflects applied, offline and failed outcomes',
    () {
      final applied = regionPresetMessage(
        const dns.RegionalPresetResult(
          ok: true,
          preset: 'Default',
          pendingUrls: <String>[],
        ),
      );
      expect(applied, '区域预置已应用：Default');

      final offline = regionPresetMessage(
        const dns.RegionalPresetResult(
          ok: true,
          preset: 'Russia',
          pendingUrls: <String>['u1'],
        ),
      );
      expect(offline, contains('离线'));
      expect(offline, contains('1 个远程模板待下载'));

      final failed = regionPresetMessage(
        const dns.RegionalPresetResult(
          ok: false,
          preset: 'Iran',
          pendingUrls: <String>[],
        ),
      );
      expect(failed, contains('失败'));
    },
  );
}

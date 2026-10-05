import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/dns.dart' as dns;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/settings/global_hotkey_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';
import 'package:v2rayn_desktop/features/settings/theme_setting_dialog.dart';

/// Open the option settings window as an independent top-level window.
///
/// The window runs in its own Flutter engine with a snapshot of the current
/// settings document. It has no Rust handle: 确定 relays the draft back here,
/// where the existing settings path persists it, syncs autostart and applies
/// the real plan before the window closes. 取消/Esc/title-bar close write
/// nothing. A failure to open is reported visibly and never faked as success.
Future<void> openOptionSettingWindow(
  BuildContext context,
  WidgetRef ref,
) async {
  try {
    ref.read(settingsControllerProvider.notifier).load();
  } catch (_) {}
  final settings = ref.read(settingsControllerProvider.notifier);
  final snapshot = settings.draft();
  final snapshotRevision = ref.read(settingsControllerProvider).revision;
  final opened = await OptionWindowHost.instance.open(
    snapshot: snapshot,
    onSave: (draftJson) =>
        _applyOptionDraft(ref, draftJson, snapshotRevision),
  );
  if (!opened && context.mounted) {
    ScaffoldMessenger.maybeOf(context)
        ?.showSnackBar(const SnackBar(content: Text('打开设置窗口失败')));
  }
}

/// Persist a draft relayed from the settings window through the same path the
/// in-process dialog uses: optimistic save, autostart sync, then await the real
/// plan apply. The result distinguishes persistence from application so a
/// failure is shown to the user instead of being reported as success. The
/// draft must submit the revision captured when the window opened
/// (AUD-DESK-02), so an older draft cannot overwrite newer edits.
Future<SettingsEditorOutcome> _applyOptionDraft(
  WidgetRef ref,
  String draftJson,
  int snapshotRevision,
) async {
  Map<String, dynamic> draft;
  try {
    final decoded = jsonDecode(draftJson);
    if (decoded is! Map) {
      return const SettingsEditorOutcome(ok: false, message: '保存配置失败');
    }
    draft = decoded.cast<String, dynamic>();
  } catch (_) {
    return const SettingsEditorOutcome(ok: false, message: '保存配置失败');
  }
  final outcome = await ref
      .read(settingsControllerProvider.notifier)
      .saveAndApply(draft, expectedRevision: snapshotRevision);
  final notice = outcome.ok && outcome.statusKey != null
      ? SettingsController.statusMessageFor(outcome.statusKey!)
      : null;
  return SettingsEditorOutcome(
    ok: outcome.ok,
    message: outcome.ok ? notice : outcome.message,
  );
}

/// Open the theme setting window (immediate apply + persist).
Future<void> openThemeSettingDialog(BuildContext context, WidgetRef ref) async {
  try {
    ref.read(settingsControllerProvider.notifier).load();
  } catch (_) {}
  await ThemeSettingDialog.show(context);
}

/// Open the global hotkey window (record as WPF Key, persist, re-register).
Future<void> openGlobalHotkeyWindow(BuildContext context, WidgetRef ref) async {
  try {
    ref.read(settingsControllerProvider.notifier).load();
  } catch (_) {}
  await GlobalHotkeyWindow.show(context);
}

// ---------------------------------------------------------------------------
// ACT-MAIN-029 / ACT-WIN-004 / ACT-MAIN-032..034 / ACT-WIN-008
// Menu actions whose command construction is pure and unit-tested. The real
// process launch goes through an injectable [DesktopLauncher] so tests record
// the argv and never touch the host.
// ---------------------------------------------------------------------------

/// Upstream `Global.RebootAs`: the self-restart marker the Windows runner
/// (`windows/runner/main.cpp`) recognises, bypassing the single-instance exit.
const String kRebootAsArgument = 'rebootas';

/// Upstream `Global.GithubUrl`.
const String kGithubUrl = 'https://github.com';

/// `Global.CoreUrls` keyed by the upstream help-menu display name
/// (`CoreType.ToString().Replace("_", " ").UpperFirstChar()`). `V2fly` and
/// `Hysteria` are excluded from the upstream help menu
/// (`MainWindow.AddHelpMenuItem`).
const Map<String, String> coreRepoSlugs = <String, String>{
  'v2rayN': '2dust/v2rayN',
  'V2fly v5': 'v2fly/v2ray-core',
  'Xray': 'XTLS/Xray-core',
  'Mihomo': 'MetaCubeX/mihomo',
  'Sing box': 'SagerNet/sing-box',
  'Hysteria2': 'apernet/hysteria',
  'Naiveproxy': 'klzgrad/naiveproxy',
  'Tuic': 'EAimTY/tuic',
  'Juicity': 'juicity/juicity',
  'Brook': 'txthinking/brook',
  'Overtls': 'ShadowsocksR-Live/overtls',
  'Shadowquic': 'spongebob888/shadowquic',
  'Mieru': 'enfein/mieru',
};

/// Core home page opened by the help menu: the release URL minus `/releases`
/// (`CoreInfoManager.GetCoreUrl` + `MainWindow.AddHelpMenuItem`).
String coreWebsiteUrl(String coreName) {
  final slug = coreRepoSlugs[coreName];
  if (slug == null) return '';
  return '$kGithubUrl/$slug';
}

/// Upstream `ProcUtils.RebootAsAdmin`: the current exe relaunched with the
/// `rebootas` argument under the Windows `runas` verb (ShellExecuteW
/// equivalent; Dart has no direct verb, so PowerShell `Start-Process -Verb
/// RunAs` is used).
class ElevationRequest {
  const ElevationRequest({
    required this.exePath,
    required this.arguments,
    required this.verb,
  });

  final String exePath;
  final List<String> arguments;
  final String verb;
}

ElevationRequest buildAdminRelaunchRequest(String exePath) => ElevationRequest(
  exePath: exePath,
  arguments: const <String>[kRebootAsArgument],
  verb: 'runas',
);

String _powerShellQuote(String value) => "'${value.replaceAll("'", "''")}'";

/// PowerShell argv that performs the elevated relaunch. A UAC cancel or launch
/// failure surfaces as a non-zero exit / `ProcessException`.
List<String> buildElevationPowerShellArgs(ElevationRequest request) {
  final argList = request.arguments.map(_powerShellQuote).join(',');
  return <String>[
    '-NoProfile',
    '-NonInteractive',
    '-Command',
    'Start-Process -FilePath ${_powerShellQuote(request.exePath)} '
        '-ArgumentList @($argList) -Verb ${request.verb}',
  ];
}

/// Process launcher seam so tests assert the argv without spawning anything.
typedef DesktopLauncher = Future<ProcessResult> Function(
  String program,
  List<String> arguments,
);

Future<ProcessResult> defaultDesktopLauncher(
  String program,
  List<String> arguments,
) => Process.run(program, arguments, runInShell: false);

/// Relaunch elevated. Returns false on a missing path, UAC cancel or failure.
Future<bool> relaunchAsAdmin({
  String? exePath,
  DesktopLauncher launcher = defaultDesktopLauncher,
}) async {
  final path = (exePath ?? Platform.resolvedExecutable).trim();
  if (path.isEmpty || !Platform.isWindows) return false;
  try {
    final result = await launcher(
      'powershell.exe',
      buildElevationPowerShellArgs(buildAdminRelaunchRequest(path)),
    );
    return result.exitCode == 0;
  } on ProcessException {
    return false;
  }
}

/// How the UWP loopback exemption is applied. Upstream ships
/// `bin/EnableLoopback.exe`; the underlying Windows primitive is
/// `CheckNetIsolation LoopbackExempt`. Both are reversible (`-d` removes).
enum LoopbackExemptionMode { add, remove }

class LoopbackExemptionCommand {
  const LoopbackExemptionCommand({
    required this.program,
    required this.arguments,
    required this.reversible,
    required this.summary,
  });

  final String program;
  final List<String> arguments;
  final bool reversible;
  final String summary;
}

/// Build the loopback-exemption command. With `bundledToolPath` the upstream
/// interactive `EnableLoopback.exe` is invoked; otherwise the equivalent,
/// reversible `CheckNetIsolation LoopbackExempt` command is constructed.
LoopbackExemptionCommand buildLoopbackExemptionCommand({
  String appContainer = '',
  LoopbackExemptionMode mode = LoopbackExemptionMode.add,
  String? bundledToolPath,
}) {
  if (appContainer.trim().isEmpty && bundledToolPath != null) {
    return LoopbackExemptionCommand(
      program: bundledToolPath,
      arguments: const <String>[],
      reversible: true,
      summary: '启用回环豁免（可用 EnableLoopback.exe 撤销）',
    );
  }
  final name = appContainer.trim().isEmpty
      ? '<PackageFamilyName>'
      : appContainer.trim();
  return LoopbackExemptionCommand(
    program: 'CheckNetIsolation.exe',
    arguments: <String>[
      'LoopbackExempt',
      mode == LoopbackExemptionMode.add ? '-a' : '-d',
      '-n=$name',
    ],
    reversible: true,
    summary: mode == LoopbackExemptionMode.add
        ? '为 $name 添加回环豁免（可撤销：-d -n=$name）'
        : '移除 $name 的回环豁免',
  );
}

/// Open a URL through the OS handler (`cmd /c start "" <url>`), matching
/// upstream `ProcUtils.ProcessStart`. The application issues no HTTP request.
List<String> buildOpenUrlArgs(String url) => <String>['/c', 'start', '', url];

Future<bool> openCoreWebsite({
  String coreName = 'Xray',
  DesktopLauncher launcher = defaultDesktopLauncher,
}) async {
  final url = coreWebsiteUrl(coreName);
  if (url.isEmpty || !Platform.isWindows) return false;
  try {
    final result = await launcher('cmd', buildOpenUrlArgs(url));
    return result.exitCode == 0;
  } on ProcessException {
    return false;
  }
}

/// Apply a regional routing/DNS preset (upstream
/// `MainWindowViewModel.ApplyRegionalPreset`). `Default` is fully local;
/// Russia/Iran persist the region source URLs plus the embedded DNS defaults
/// offline (no application-issued network request) and report pending remote
/// templates honestly.
String applyRegionPreset(WidgetRef ref, String preset) => regionPresetMessage(
  ref.read(bridgePortProvider).applyRegionalPreset(preset),
);

/// User-facing message for a regional-preset outcome. Pure so the wording is
/// unit-testable without a widget or the native bridge.
String regionPresetMessage(dns.RegionalPresetResult result) {
  if (!result.ok) {
    return '区域预置失败：${result.error?.code ?? 'unknown'}';
  }
  if (result.pendingUrls.isEmpty) {
    return '区域预置已应用：${result.preset}';
  }
  return '区域预置已应用（离线）：${result.preset}，${result.pendingUrls.length} 个远程模板待下载';
}

// R4-32 contract: the official Windows delivery package is built unarmed and
// the installer/self-update surface stays consistent with the release tooling.
//
// These are static assertions over the release scripts and the Inno script so
// the delivery contract is pinned in CI without a full build or a host
// install/uninstall. The runtime packaged first-run/reopen evidence lives in
// tools/release/r4_32_real_entry.ps1 and the real-UI integration tests.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

Directory _repoRoot() {
  // flutter test runs with cwd = apps/desktop.
  final root = Directory.current.parent.parent;
  if (!File('${root.path}${Platform.pathSeparator}pubspec.yaml').existsSync() &&
      !File('${root.path}${Platform.pathSeparator}Cargo.toml').existsSync()) {
    throw StateError('repo root not found from ${Directory.current.path}');
  }
  return root;
}

String _read(Directory root, String rel) {
  final f = File(
    '${root.path}${Platform.pathSeparator}${rel.replaceAll('/', Platform.pathSeparator)}',
  );
  expect(f.existsSync(), isTrue, reason: 'missing $rel');
  return f.readAsStringSync();
}

String _pubspecVersion(Directory root) {
  final line = _read(
    root,
    'apps/desktop/pubspec.yaml',
  ).split('\n').firstWhere((l) => l.startsWith('version:'));
  return line.split(':')[1].trim();
}

void main() {
  final root = _repoRoot();

  test('release build defaults to unarmed (no SMOKE_ARMED define)', () {
    final build = _read(root, 'tools/release/build_windows.ps1');
    // The arming define is only appended behind the -SmokeArmed switch.
    expect(build.contains('V2RAYN_R_SMOKE_ARMED=true'), isTrue);
    expect(build.contains('if (\$SmokeArmed) { \$flutterArgs += '), isTrue);
    expect(build.contains('smoke_armed    = [bool]\$SmokeArmed'), isTrue);
    expect(build.contains('git_dirty      = \$gitDirty'), isTrue);
    // Armed artifacts must never overwrite the official dist package.
    expect(build.contains(r"Join-Path $DistDir 'evidence-armed'"), isTrue);
  });

  test(
    'self-update helper ships under the runner name the updater launches',
    () {
      final build = _read(root, 'tools/release/build_windows.ps1');
      final destLine = build
          .split('\n')
          .firstWhere(
            (l) =>
                l.contains('upgrade_runner.exe') && l.contains('-Destination'),
            orElse: () => '',
          );
      expect(
        destLine.contains('v2rayN-upgrade.exe'),
        isTrue,
        reason: 'upgrade_runner must be copied to v2rayN-upgrade.exe',
      );
      final updater = _read(root, 'crates/updater/src/app_upgrade.rs');
      expect(
        RegExp(r'DEFAULT_RUNNER_NAME:\s*&str\s*=\s*"v2rayN-upgrade\.exe"')
            .hasMatch(updater),
        isTrue,
      );
    },
  );

  test('installer version and output name track the pubspec version', () {
    final iss = _read(root, 'tools/release/v2rayn-r.iss');
    final version = _pubspecVersion(root);
    expect(
      iss.contains('#define MyAppVersion "$version"'),
      isTrue,
      reason: 'iss MyAppVersion must match pubspec $version',
    );
    expect(
      iss.contains('OutputBaseFilename=v2rayN-R-$version-windows-x64-setup'),
      isTrue,
    );
    expect(
      iss.contains(
        r'MySourceDir "..\..\dist\v2rayN-R-'
        '$version-windows-x64"',
      ),
      isTrue,
    );
  });

  test('uninstall never recursively deletes the whole install root', () {
    final iss = _read(root, 'tools/release/v2rayn-r.iss');
    final lines = iss.split('\n').map((l) => l.trim());
    var inSection = false;
    final rules = <String>[];
    for (final raw in lines) {
      if (raw.startsWith('[') && raw.endsWith(']')) {
        inSection = raw == '[UninstallDelete]';
        continue;
      }
      if (inSection && raw.isNotEmpty && !raw.startsWith(';')) {
        rules.add(raw);
      }
    }
    expect(rules, isNotEmpty);
    for (final rule in rules) {
      final recursiveRoot =
          rule.contains('filesandordirs') &&
          RegExp(r'Name:\s*"\s*\{app\}\s*\*?\s*"').hasMatch(rule);
      expect(recursiveRoot, isFalse, reason: 'recursive {app} delete: $rule');
    }
    expect(
      rules.any((r) => r.contains('dirifempty') && r.contains('{app}')),
      isTrue,
      reason: 'install dir must only be removed when empty',
    );
    for (final managed in [r'{app}\.staging', r'{app}\app.previous']) {
      expect(
        rules.any((r) => r.contains(managed)),
        isTrue,
        reason: 'missing managed leftover rule $managed',
      );
    }
  });

  test('delivery scripts never listen on the live 10808', () {
    for (final rel in [
      'tools/release/build_windows.ps1',
      'tools/release/negative_unarmed.ps1',
      'tools/release/install_test.ps1',
      'tools/release/selfupdate_test.ps1',
    ]) {
      final text = _read(root, rel);
      expect(
        text.contains('LocalPort 10808'),
        isFalse,
        reason: '$rel must never check/bind the live proxy port 10808',
      );
    }
    // The negative test targets the >=11808 test floor instead.
    expect(
      _read(
        root,
        'tools/release/negative_unarmed.ps1',
      ).contains('LocalPort 11808'),
      isTrue,
    );
  });
}

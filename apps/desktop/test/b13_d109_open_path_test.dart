// B13 D1-09 / D-LEFT: the custom-node "open externally" path only accepts
// plain drive-letter absolute paths (no shell left in the launch path, so `!`
// is harmless); everything else fails safe before any process spawns.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/custom_editor_dialog.dart';

void main() {
  test('isSafeExternalOpenPath rejects shell metacharacters', () {
    expect(isSafeExternalOpenPath(''), isFalse);
    expect(isSafeExternalOpenPath('C:\\x" & calc & "'), isFalse);
    expect(isSafeExternalOpenPath('C:\\x|whoami'), isFalse);
    expect(isSafeExternalOpenPath('C:\\x;whoami'), isFalse);
    expect(isSafeExternalOpenPath('C:\\x`whoami`'), isFalse);
    expect(isSafeExternalOpenPath('C:\\x%PATH%'), isFalse);
    expect(isSafeExternalOpenPath('C:\\x^c'), isFalse);
    expect(isSafeExternalOpenPath('C:\\x\nevil'), isFalse);
  });

  test('isSafeExternalOpenPath accepts ordinary absolute config paths', () {
    expect(isSafeExternalOpenPath('C:\\data\\config\\my file.json'), isTrue);
    expect(isSafeExternalOpenPath('C:\\data\\a!b.json'), isTrue);
    expect(isSafeExternalOpenPath('D:/data/config/node.json'), isTrue);
  });

  test('isSafeExternalOpenPath rejects UNC, flags and bare names', () {
    expect(isSafeExternalOpenPath('\\\\srv\\share\\my file (1).json'), isFalse);
    expect(isSafeExternalOpenPath('my-node.json'), isFalse);
    expect(isSafeExternalOpenPath('/etc/node.json'), isFalse);
    expect(isSafeExternalOpenPath('-node.json'), isFalse);
  });
}

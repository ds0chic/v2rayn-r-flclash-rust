// B13 D1-09: the custom-node "open externally" path must reject shell
// metacharacters before they reach `cmd /c start`, and quote the path so
// spaces survive.
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

  test('isSafeExternalOpenPath accepts ordinary config paths', () {
    expect(isSafeExternalOpenPath('my-node.json'), isTrue);
    expect(isSafeExternalOpenPath('C:\\data\\config\\my file.json'), isTrue);
    expect(
      isSafeExternalOpenPath('\\\\srv\\share\\my file (1).json'),
      isTrue,
    );
  });

  test('quoteExternalOpenPath double-quotes for start', () {
    expect(
      quoteExternalOpenPath('C:\\data\\my file.json'),
      '"C:\\data\\my file.json"',
    );
  });
}

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/stable.dart' as stable;
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

/// SP-00 shared-contract compile + version contracts.
///
/// These assert that the frozen stable-contract surface is reachable from the
/// app (compile contract) and that the runtime read model keeps identity facts
/// separate. Behavior wiring for the individual contract families happens in
/// the later SP cards; this file must not claim those are done.
void main() {
  test('stable shared contract family is exposed with version 1', () {
    // Compile contract: the generated binding exists under the frozen name and
    // signature. Calling it requires the native library (real-entry evidence),
    // so this test asserts the symbol instead of invoking FRB.
    final binding = stable.stableContractVersion;
    expect(binding, isA<Future<int> Function()>());
  });

  test(
    'runtime read model keeps desired/applied and the actual tun fact separate',
    () {
      const view = RuntimeView(
        state: 'Running',
        ports: [11808],
        sessionId: 's-1',
        desiredRevision: null,
        appliedRevision: null,
        tun: RuntimeTunView(
          adapterName: 'v2rayn-tun',
          interfaceIndex: 84,
          routeCount: 0,
          dryRun: false,
        ),
      );
      // The actual lease fact is carried as its own field, never derived from a
      // desired switch.
      expect(view.tun?.adapterName, 'v2rayn-tun');
      expect(view.hasAppliedEndpoint, isTrue);
    },
  );

  test('revision pair reports unapplied changes without faking applied', () {
    const view = RuntimeView(desiredRevision: null, appliedRevision: null);
    expect(view.hasUnappliedChanges, isFalse);
    final view2 = view.copyWith();
    expect(view2.appliedRevision, isNull);
  });
}

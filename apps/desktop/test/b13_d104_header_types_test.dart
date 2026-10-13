// B13 D1-04: raw/kcp camouflage candidates must match the upstream
// AddServerWindow combos: raw = [none, http], kcp = [none] + KcpHeaderTypes
// (which includes dns and excludes http).
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';

List<String?> dropdownValues(String network, String key) {
  final field = transportFields(network)
      .firstWhere((f) => f.key == key, orElse: () => throw StateError(key));
  return [for (final o in field.options ?? const []) o.value];
}

void main() {
  test('kcp header offers dns and not http', () {
    final values = dropdownValues('kcp', 'kcpHeaderType');
    expect(values, contains('dns'));
    expect(values, isNot(contains('http')));
    expect(values.first, 'none');
  });

  test('raw header offers exactly none/http', () {
    final values = dropdownValues('raw', 'rawHeaderType');
    expect(values, <String?>['none', 'http']);
  });
}

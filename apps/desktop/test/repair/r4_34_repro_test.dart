// R4-34 repro: saving a resource source or the global `GuiItem.AutoUpdateInterval`
// must make the Geo/SRS download pass actually schedulable on the upstream
// cadence (D32: the period was stored but had no TaskManager consumer). This
// file intentionally asserts the fixed contract; it must fail against the
// pre-fix implementation (the resource contract did not exist).
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/resource_auto_update.dart';

Map<String, dynamic> settingsWith({int interval = 1}) => <String, dynamic>{
  'GuiItem': <String, dynamic>{'AutoUpdateInterval': interval},
  'ConstItem': <String, dynamic>{
    'GeoSourceUrl': 'https://mirror.example/{0}.dat',
    'SrsSourceUrl': 'https://mirror.example/rule-set/{1}.srs',
  },
};

void main() {
  test(
    'a saved global period schedules the Geo pass on a positive cadence',
    () {
      final plan = resourceAutoUpdatePlan(settingsWith());
      expect(plan.enabled, isTrue);
      expect(plan.hasSources, isTrue);
      expect(plan.active, isTrue);
      expect(resourcePassDue(plan, 0), isFalse, reason: 'startup tick');
      expect(resourcePassDue(plan, 1), isTrue);
    },
  );

  test('AutoUpdateInterval 0 disables the resource task', () {
    final plan = resourceAutoUpdatePlan(settingsWith(interval: 0));
    expect(plan.enabled, isFalse);
    expect(resourcePassDue(plan, 1), isFalse);
  });

  test('a 3h cadence fires only on positive multiples', () {
    final plan = resourceAutoUpdatePlan(settingsWith(interval: 3));
    expect(resourcePassDue(plan, 1), isFalse);
    expect(resourcePassDue(plan, 3), isTrue);
    expect(resourcePassDue(plan, 6), isTrue);
    expect(resourcePassDue(plan, 7), isFalse);
  });
}

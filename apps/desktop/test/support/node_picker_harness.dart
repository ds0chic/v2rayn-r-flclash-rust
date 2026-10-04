// Shared harness for the RE-PROF-11 node picker tests.
//
// The locked Flutter build leaks native resources per `pumpWidget`, so each
// test file must build the picker at most once (single page build). This file
// is intentionally not named `*_test.dart` so it is never run as a suite.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/speedtest.dart' as speedtest;
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';

/// Synthetic stored profile for the picker (public template fields only).
c.ProfileDto pickerNode(
  String id, {
  required String remarks,
  ConfigType type = ConfigType.vless,
  String subid = 'sub-1',
  String address = '192.0.2.1',
  int port = 443,
  String network = 'raw',
  String? tls,
}) => c.ProfileDto(
  indexId: id,
  configType: type,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: subid,
  isSub: true,
  displayLog: true,
  remarks: remarks,
  address: address,
  port: port,
  password: '',
  username: '',
  network: network,
  security: c.SecurityDto(streamSecurity: tls),
  protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
  transportExtra: const c.TransportExtraDto(extraJson: '{}'),
  extraJson: '{}',
);

c.SubItemDto pickerSub(String id, String remarks) => c.SubItemDto(
  id: id,
  remarks: remarks,
  url: 'https://example.com/$id',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 1,
  autoUpdateInterval: 0,
  updateTime: 0,
);

/// Build the picker once and run [onResult] when it closes.
Future<void> pumpNodePicker(
  WidgetTester tester, {
  required List<c.ProfileDto> candidates,
  List<c.SubItemDto>? subItems,
  bool multiSelect = true,
  List<ConfigType>? filterConfigTypes,
  bool filterExclude = false,
  String? currentGroupSubId,
  List<speedtest.SpeedTestResultDto>? speedResults,
  required void Function(List<String>?) onResult,
}) async {
  tester.view.physicalSize = const Size(1400, 1024);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (context) => Center(
            child: ElevatedButton(
              onPressed: () async {
                final result = await showNodePicker(
                  context,
                  candidates: candidates,
                  subItems: subItems,
                  multiSelect: multiSelect,
                  filterConfigTypes: filterConfigTypes,
                  filterExclude: filterExclude,
                  currentGroupSubId: currentGroupSubId,
                  speedResults: speedResults,
                );
                onResult(result);
              },
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
}

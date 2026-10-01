import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

/// A synthetic bridge seeded with a couple of subscriptions for widget tests.
class SeededSubsBridge extends SyntheticBridgePort {
  SeededSubsBridge() {
    saveSubItem(
      c.SubItemDto(
        id: '',
        remarks: '测试订阅',
        url: 'https://example.com/sub',
        moreUrl: '',
        enabled: true,
        userAgent: '',
        sort: 0,
        autoUpdateInterval: 60,
        updateTime: 0,
      ),
    );
  }
}

/// Provider container with the synthetic bridge and in-memory UI store.
ProviderContainer makeSubsContainer({BridgePort? bridge}) {
  return ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge ?? SeededSubsBridge()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(50),
    ],
  );
}

/// A minimal ConsumerWidget that exposes its [WidgetRef] to a test callback so
/// actions taking a `WidgetRef` can be driven without the full app shell.
class RefProbe extends ConsumerWidget {
  const RefProbe({
    super.key,
    required this.onRef,
    this.child = const SizedBox(),
  });

  final void Function(BuildContext context, WidgetRef ref) onRef;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    onRef(context, ref);
    return child;
  }
}

SubsState readSubsState(ProviderContainer container) =>
    container.read(subsControllerProvider);

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_page.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Tab definitions mirroring the upstream TabControl order
/// (compat/layouts.yaml LAY-MAIN-001/002/003):
///   信息 (MsgView) / 当前代理 (ClashProxies) / 当前连接 (ClashConnections)
/// and, in tab layout, a leading 配置项 (profiles) tab.
enum AppTab {
  profiles('profiles', '配置项', Icons.dns_outlined),
  info('info', '信息', Icons.message_outlined),
  proxies('proxies', '当前代理', Icons.call_split_outlined),
  connections('connections', '当前连接', Icons.lan_outlined);

  const AppTab(this.id, this.label, this.icon);

  final String id;
  final String label;
  final IconData icon;
}

enum TabStripPlacement { top, left }

class SideTabs extends ConsumerWidget {
  const SideTabs({
    super.key,
    required this.tabs,
    required this.placement,
    required this.tabIndex,
    required this.onTabSelected,
  });

  final List<AppTab> tabs;
  final TabStripPlacement placement;
  final int tabIndex;
  final ValueChanged<int> onTabSelected;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final index = tabIndex.clamp(0, tabs.length - 1);
    final strip = _TabStrip(
      tabs: tabs,
      index: index,
      placement: placement,
      onTabSelected: onTabSelected,
    );
    final content = Expanded(
      key: ValueKey<String>('main-tab-content-${tabs[index].id}'),
      child: _contentFor(tabs[index]),
    );
    if (placement == TabStripPlacement.top) {
      return Column(children: <Widget>[strip, content]);
    }
    return Row(
      children: <Widget>[strip, const VerticalDivider(width: 1), content],
    );
  }

  Widget _contentFor(AppTab tab) {
    switch (tab) {
      case AppTab.profiles:
        return const ProfilesPage();
      case AppTab.info:
        return const _InfoPlaceholder();
      case AppTab.proxies:
        return const _ClashPlaceholder(title: '当前代理');
      case AppTab.connections:
        return const _ClashPlaceholder(title: '当前连接');
    }
  }
}

class _TabStrip extends StatelessWidget {
  const _TabStrip({
    required this.tabs,
    required this.index,
    required this.placement,
    required this.onTabSelected,
  });

  final List<AppTab> tabs;
  final int index;
  final TabStripPlacement placement;
  final ValueChanged<int> onTabSelected;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final buttons = <Widget>[
      for (var i = 0; i < tabs.length; i++)
        _TabButton(
          key: ValueKey<String>('main-tab-${tabs[i].id}'),
          tab: tabs[i],
          selected: i == index,
          vertical: placement == TabStripPlacement.left,
          onTap: () => onTabSelected(i),
        ),
    ];
    if (placement == TabStripPlacement.top) {
      return Material(
        color: scheme.surfaceContainer,
        child: SizedBox(
          height: AppTokens.toolbarHeight,
          child: Row(children: buttons),
        ),
      );
    }
    return Material(
      color: scheme.surfaceContainer,
      child: SizedBox(width: 120, child: Column(children: buttons)),
    );
  }
}

class _TabButton extends StatelessWidget {
  const _TabButton({
    super.key,
    required this.tab,
    required this.selected,
    required this.vertical,
    required this.onTap,
  });

  final AppTab tab;
  final bool selected;
  final bool vertical;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final content = Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Icon(tab.icon, size: 16),
        const SizedBox(width: 6),
        Text(tab.label, style: const TextStyle(fontSize: 12)),
      ],
    );
    return InkWell(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        alignment: vertical ? Alignment.centerLeft : Alignment.center,
        decoration: BoxDecoration(
          color: selected ? scheme.surface : null,
          border: Border(
            bottom: vertical
                ? BorderSide.none
                : (selected
                      ? BorderSide(color: scheme.primary, width: 2)
                      : BorderSide(color: scheme.outlineVariant)),
            left: vertical && selected
                ? BorderSide(color: scheme.primary, width: 3)
                : BorderSide.none,
          ),
        ),
        child: content,
      ),
    );
  }
}

class _InfoPlaceholder extends StatelessWidget {
  const _InfoPlaceholder();

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.all(6),
          child: Row(
            children: const <Widget>[
              SizedBox(width: 24),
              Icon(Icons.search, size: 14),
              SizedBox(width: 6),
              Text('过滤', style: TextStyle(fontSize: 12)),
              Spacer(),
              Text('自动刷新', style: TextStyle(fontSize: 12)),
            ],
          ),
        ),
        const Divider(height: 1),
        const Expanded(
          child: Padding(
            padding: EdgeInsets.all(12),
            child: Text(
              '信息：内核日志尚未接入（T05 占位，未运行）',
              key: ValueKey<String>('info-placeholder-text'),
              style: TextStyle(fontSize: 12),
            ),
          ),
        ),
      ],
    );
  }
}

class _ClashPlaceholder extends StatelessWidget {
  const _ClashPlaceholder({required this.title});

  final String title;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(
        '$title：未接入 Clash API',
        key: const ValueKey<String>('clash-placeholder-text'),
        style: const TextStyle(fontSize: 12),
      ),
    );
  }
}

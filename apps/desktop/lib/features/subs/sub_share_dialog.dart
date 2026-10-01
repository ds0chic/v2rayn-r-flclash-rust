import 'package:flutter/material.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;

/// Renders a subscription's download URL as a scannable QR code (F-SUB-001 /
/// upstream `SubSettingViewModel.ShareSub`). Screen-camera and image scanning
/// remain out of scope this round (`docs/evidence/T09-wiring.md`).
Future<void> showSubShareDialog(BuildContext context, c.SubItemDto item) {
  return showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      key: const ValueKey('sub-share-dialog'),
      title: const Text('分享订阅', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 320,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Text(
              item.remarks.isEmpty ? item.url : item.remarks,
              style: const TextStyle(fontSize: 13),
            ),
            const SizedBox(height: 12),
            if (item.url.trim().isEmpty)
              const Text('该订阅没有可分享的 URL', style: TextStyle(fontSize: 12))
            else
              SizedBox(
                key: const ValueKey('sub-share-qr'),
                width: 236,
                height: 236,
                child: ColoredBox(
                  color: Colors.white,
                  child: QrImageView(
                    data: item.url,
                    version: QrVersions.auto,
                    size: 220,
                    backgroundColor: Colors.white,
                  ),
                ),
              ),
            const SizedBox(height: 12),
            Text(
              item.url,
              style: const TextStyle(fontSize: 11),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('sub-share-close'),
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('关闭'),
        ),
      ],
    ),
  );
}

import 'dart:typed_data';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:image/image.dart' as img;
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:zxing2/qrcode.dart';

/// ACT-MAIN-018 / F-IMPORT-006: 扫描图片中的二维码 → 导入.
///
/// Upstream `MainWindowViewModel.AddServerViaImageAsync` opens a file dialog
/// (`PNG|*.png|All|*.*`), calls `QRCodeUtils.ParseBarcode(file)`, and feeds the
/// decoded text to `ConfigHandler.AddBatchServers`; a cancelled dialog returns
/// silently and an image without a decodable QR only notices
/// `NoValidQRcodeFound`. This module keeps that contract on the existing
/// share-text import pipeline ([importShareText]).
///
/// Screen capture scanning (upstream `Ctrl+S` / ACT-MAIN-017) is deliberately
/// not implemented here; it is tracked as FIX-05B.
typedef QrImagePicker = Future<Uint8List?> Function();

/// File extensions offered by the image picker.
const List<String> qrImageExtensions = <String>[
  'png',
  'jpg',
  'jpeg',
  'bmp',
  'gif',
  'webp',
];

/// Default picker: the real file dialog. Returns null when the user cancels.
Future<Uint8List?> pickQrImageFile() async {
  const group = XTypeGroup(label: '图片', extensions: qrImageExtensions);
  final file = await openFile(acceptedTypeGroups: <XTypeGroup>[group]);
  if (file == null) return null;
  return file.readAsBytes();
}

/// Decodes a QR payload from encoded image bytes (PNG/JPEG/...).
///
/// Returns null for unreadable files, images without a QR code, or empty
/// payloads. Mirrors upstream `QRCodeUtils.ReaderBarcode`: one direct pass,
/// then a horizontally flipped retry for mirrored codes. Like upstream
/// `ParseBarcode` this runs synchronously on the calling isolate.
String? decodeQrImage(Uint8List bytes) {
  if (bytes.isEmpty) return null;
  img.Image? decoded;
  try {
    decoded = img.decodeImage(bytes);
  } catch (_) {
    return null;
  }
  if (decoded == null) return null;
  final direct = _decodeWithZxing(decoded);
  if (direct != null && direct.trim().isNotEmpty) return direct;
  img.Image flipped;
  try {
    flipped = img.flip(decoded, direction: img.FlipDirection.horizontal);
  } catch (_) {
    return null;
  }
  final mirrored = _decodeWithZxing(flipped);
  return (mirrored != null && mirrored.trim().isNotEmpty) ? mirrored : null;
}

String? _decodeWithZxing(img.Image image) {
  try {
    final rgba = image.getBytes(order: img.ChannelOrder.rgba);
    final pixels = Int32List(image.width * image.height);
    for (var i = 0; i < pixels.length; i++) {
      final r = rgba[i * 4];
      final g = rgba[i * 4 + 1];
      final b = rgba[i * 4 + 2];
      pixels[i] = (r << 16) | (g << 8) | b;
    }
    final source = RGBLuminanceSource(image.width, image.height, pixels);
    final bitmap = BinaryBitmap(HybridBinarizer(source));
    return QRCodeReader().decode(bitmap).text;
  } catch (_) {
    return null;
  }
}

/// ACT-MAIN-018: choose an image, decode its QR code, and hand the text to the
/// existing import pipeline. Cancelling is silent; errors are actionable.
Future<void> scanImageQr(
  BuildContext context,
  WidgetRef ref, {
  QrImagePicker picker = pickQrImageFile,
}) async {
  Uint8List? bytes;
  try {
    bytes = await picker();
  } catch (_) {
    _toast(ref, '打开图片失败，请重试');
    return;
  }
  if (bytes == null) return; // Cancelled: no error, no state change.
  if (bytes.isEmpty) {
    _toast(ref, '所选图片为空，请重新选择');
    return;
  }
  final text = decodeQrImage(bytes);
  if (text == null || text.trim().isEmpty) {
    _toast(ref, '扫描完成，未发现有效二维码（请选择含清晰二维码的 PNG/JPG 图片）');
    return;
  }
  if (!context.mounted) return;
  await importShareText(context, ref, text, sourceLabel: '扫描');
}

void _toast(WidgetRef ref, String message) {
  ref.read(uiShellControllerProvider.notifier).setMessage(message);
}

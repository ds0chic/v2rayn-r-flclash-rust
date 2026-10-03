import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:image/image.dart' as img;
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/subs/scan_image_qr.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:window_manager/window_manager.dart';

/// ACT-MAIN-017 / F-IMPORT-006: 扫描屏幕上的二维码 → 导入.
///
/// Upstream `MainWindow.xaml.cs:265 ScanScreenTaskAsync` hides the main window,
/// captures the whole work area (`QRCodeWindowsUtils.CaptureScreen`), feeds the
/// PNG to `MainWindowViewModel.ScanScreenResult`, then restores the window. The
/// same command is triggered by the `配置项 → 扫描屏幕上的二维码` menu entry and
/// by `Ctrl+S`. This module keeps that contract on the existing share-text
/// pipeline ([importShareText]) and reuses the FIX-05 decoder ([decodeQrImage]).
///
/// Unlike the WPF original (which restores the window on the success path only),
/// every error and cancel branch restores the window through `finally`.
typedef ScreenCapture = Future<Uint8List?> Function();

/// Injectable window visibility control so tests can assert hide/show order
/// without touching the real OS window.
abstract interface class ScanWindowControl {
  Future<void> hide();

  Future<void> show();
}

/// Real control backed by `window_manager`.
class WindowManagerScanWindowControl implements ScanWindowControl {
  const WindowManagerScanWindowControl();

  @override
  Future<void> hide() => windowManager.hide();

  @override
  Future<void> show() async {
    await windowManager.show();
    await windowManager.focus();
  }
}

/// ACT-MAIN-017: hide the window, capture the screen, decode the QR while
/// hidden, restore the window, then import through the shared pipeline.
///
/// Branch semantics:
///  * capture returns `null` → cancelled: silent, no state change;
///  * capture throws → 「屏幕截图失败，请重试」;
///  * image has no decodable QR → 「扫描完成，未发现有效二维码…」;
///  * success → [importShareText] (existing `importFromText` + persist path).
Future<void> scanScreenQr(
  BuildContext context,
  WidgetRef ref, {
  ScreenCapture capture = capturePrimaryScreenPng,
  ScanWindowControl window = const WindowManagerScanWindowControl(),
  Duration settle = const Duration(milliseconds: 180),
}) async {
  Uint8List? bytes;
  String? decoded;
  Object? failure;

  await window.hide();
  try {
    // Give the compositor a moment to actually remove the window before the
    // capture, matching the original hide-then-capture ordering.
    if (settle > Duration.zero) {
      await Future<void>.delayed(settle);
    }
    bytes = await capture();
    if (bytes != null && bytes.isNotEmpty) {
      decoded = decodeQrImage(bytes);
    }
  } catch (error) {
    failure = error;
  } finally {
    await window.show();
  }

  if (failure != null) {
    _toast(ref, '屏幕截图失败，请重试');
    return;
  }
  if (bytes == null) return; // Cancelled: silent, no state change.
  if (bytes.isEmpty) {
    _toast(ref, '屏幕截图失败，请重试');
    return;
  }
  if (decoded == null || decoded.trim().isEmpty) {
    _toast(ref, '扫描完成，未发现有效二维码（请让二维码清晰完整地显示在屏幕上后重试）');
    return;
  }
  if (!context.mounted) return;
  await importShareText(context, ref, decoded, sourceLabel: '屏幕扫描');
}

void _toast(WidgetRef ref, String message) {
  ref.read(uiShellControllerProvider.notifier).setMessage(message);
}

/// Raised by [capturePrimaryScreenPng] when the screen cannot be captured.
class ScreenCaptureException implements Exception {
  const ScreenCaptureException(this.message);

  final String message;

  @override
  String toString() => 'ScreenCaptureException: $message';
}

/// Captures the primary monitor and returns a PNG. Never returns `null`; a
/// failure raises [ScreenCaptureException] so callers can report it instead of
/// silently pretending the user cancelled.
Future<Uint8List> capturePrimaryScreenPng() async {
  if (!Platform.isWindows) {
    throw const ScreenCaptureException('screen capture is Windows-only');
  }
  final png = _captureWindowsPng();
  if (png == null) {
    throw const ScreenCaptureException('BitBlt/GetDIBits failed');
  }
  return png;
}

const int _srccopy = 0x00CC0020;
const int _captureblt = 0x40000000;
const int _dibRgbColors = 0;
const int _smCxScreen = 0;
const int _smCyScreen = 1;

DynamicLibrary? _user32Lib;
DynamicLibrary? _gdi32Lib;
DynamicLibrary get _user32 => _user32Lib ??= DynamicLibrary.open('user32.dll');
DynamicLibrary get _gdi32 => _gdi32Lib ??= DynamicLibrary.open('gdi32.dll');

typedef _GetSystemMetricsNative = Int32 Function(Int32);
typedef _GetSystemMetricsDart = int Function(int);
typedef _GetDcNative = IntPtr Function(IntPtr);
typedef _GetDcDart = int Function(int);
typedef _ReleaseDcNative = Int32 Function(IntPtr, IntPtr);
typedef _ReleaseDcDart = int Function(int, int);
typedef _CreateCompatibleDcNative = IntPtr Function(IntPtr);
typedef _CreateCompatibleDcDart = int Function(int);
typedef _CreateCompatibleBitmapNative = IntPtr Function(IntPtr, Int32, Int32);
typedef _CreateCompatibleBitmapDart = int Function(int, int, int);
typedef _SelectObjectNative = IntPtr Function(IntPtr, IntPtr);
typedef _SelectObjectDart = int Function(int, int);
typedef _BitBltNative = Int32 Function(
  IntPtr,
  Int32,
  Int32,
  Int32,
  Int32,
  IntPtr,
  Int32,
  Int32,
  Uint32,
);
typedef _BitBltDart = int Function(int, int, int, int, int, int, int, int, int);
typedef _GetDIBitsNative = Int32 Function(
  IntPtr,
  IntPtr,
  Uint32,
  Uint32,
  Pointer<Void>,
  Pointer<Void>,
  Uint32,
);
typedef _GetDIBitsDart = int Function(
  int,
  int,
  int,
  int,
  Pointer<Void>,
  Pointer<Void>,
  int,
);
typedef _DeleteObjectNative = Int32 Function(IntPtr);
typedef _DeleteObjectDart = int Function(int);
typedef _DeleteDcNative = Int32 Function(IntPtr);
typedef _DeleteDcDart = int Function(int);

/// BitBlt the primary screen into a top-down 32bpp DIB and encode it as PNG.
Uint8List? _captureWindowsPng() {
  try {
    final getSystemMetrics = _user32
        .lookupFunction<_GetSystemMetricsNative, _GetSystemMetricsDart>(
          'GetSystemMetrics',
        );
    final width = getSystemMetrics(_smCxScreen);
    final height = getSystemMetrics(_smCyScreen);
    if (width <= 0 || height <= 0) return null;

    final getDc = _user32.lookupFunction<_GetDcNative, _GetDcDart>('GetDC');
    final releaseDc = _user32.lookupFunction<_ReleaseDcNative, _ReleaseDcDart>(
      'ReleaseDC',
    );
    final createCompatibleDc = _gdi32
        .lookupFunction<_CreateCompatibleDcNative, _CreateCompatibleDcDart>(
          'CreateCompatibleDC',
        );
    final createCompatibleBitmap = _gdi32
        .lookupFunction<
          _CreateCompatibleBitmapNative,
          _CreateCompatibleBitmapDart
        >('CreateCompatibleBitmap');
    final selectObject = _gdi32
        .lookupFunction<_SelectObjectNative, _SelectObjectDart>('SelectObject');
    final bitBlt = _gdi32.lookupFunction<_BitBltNative, _BitBltDart>('BitBlt');
    final getDIBits = _gdi32.lookupFunction<_GetDIBitsNative, _GetDIBitsDart>(
      'GetDIBits',
    );
    final deleteObject = _gdi32
        .lookupFunction<_DeleteObjectNative, _DeleteObjectDart>('DeleteObject');
    final deleteDc = _gdi32.lookupFunction<_DeleteDcNative, _DeleteDcDart>(
      'DeleteDC',
    );

    final screenDc = getDc(0);
    if (screenDc == 0) return null;

    var memDc = 0;
    var bitmap = 0;
    var oldBitmap = 0;
    Pointer<Uint8>? header;
    Pointer<Uint8>? pixels;
    try {
      memDc = createCompatibleDc(screenDc);
      if (memDc == 0) return null;
      bitmap = createCompatibleBitmap(screenDc, width, height);
      if (bitmap == 0) return null;
      oldBitmap = selectObject(memDc, bitmap);
      final ok = bitBlt(
        memDc,
        0,
        0,
        width,
        height,
        screenDc,
        0,
        0,
        _srccopy | _captureblt,
      );
      if (ok == 0) return null;
      // The bitmap must not be selected into a DC while GetDIBits reads it.
      selectObject(memDc, oldBitmap);
      oldBitmap = 0;

      final imageSize = width * height * 4;
      header = calloc<Uint8>(40);
      final h = ByteData.sublistView(header.asTypedList(40));
      h.setUint32(0, 40, Endian.little);
      h.setInt32(4, width, Endian.little);
      h.setInt32(8, -height, Endian.little); // negative: top-down rows.
      h.setUint16(12, 1, Endian.little);
      h.setUint16(14, 32, Endian.little);
      h.setUint32(16, 0, Endian.little); // BI_RGB
      h.setUint32(20, 0, Endian.little);
      h.setInt32(24, 0, Endian.little);
      h.setInt32(28, 0, Endian.little);
      h.setUint32(32, 0, Endian.little);
      h.setUint32(36, 0, Endian.little);

      pixels = calloc<Uint8>(imageSize);
      final lines = getDIBits(
        screenDc,
        bitmap,
        0,
        height,
        pixels.cast<Void>(),
        header.cast<Void>(),
        _dibRgbColors,
      );
      if (lines == 0) return null;

      final raw = Uint8List.fromList(pixels.asTypedList(imageSize));
      for (var i = 3; i < raw.length; i += 4) {
        raw[i] = 255; // GetDIBits leaves alpha undefined; force opaque.
      }
      final image = img.Image.fromBytes(
        width: width,
        height: height,
        bytes: raw.buffer,
        numChannels: 4,
        order: img.ChannelOrder.bgra,
      );
      return Uint8List.fromList(img.encodePng(image));
    } finally {
      if (pixels != null) calloc.free(pixels);
      if (header != null) calloc.free(header);
      if (oldBitmap != 0) selectObject(memDc, oldBitmap);
      if (bitmap != 0) deleteObject(bitmap);
      if (memDc != 0) deleteDc(memDc);
      if (screenDc != 0) releaseDc(0, screenDc);
    }
  } catch (_) {
    return null;
  }
}

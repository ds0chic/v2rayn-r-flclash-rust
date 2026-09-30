import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/profiles.dart' as rust;

/// Thin, testable seam over the flutter_rust_bridge generated API.
///
/// The application always uses [FrbBridgePort]; [SyntheticBridgePort] only
/// exists so widget tests can run without loading the native library.
abstract class BridgePort {
  Future<void> init();

  List<ProfileSummary> generate(int count);

  int rustProfileCount();

  int pingProfile(String id);

  Stream<rust.ProgressEvent> progressStream(int count);

  rust.UiEventAck echoEvent(int seq, String kind);

  Future<int> simulateBlocking(int ms);
}

class FrbBridgePort implements BridgePort {
  const FrbBridgePort();

  @override
  Future<void> init() => RustBridgeInit.init();

  @override
  List<ProfileSummary> generate(int count) =>
      rust.generateProfiles(count: count);

  @override
  int rustProfileCount() => rust.rustProfileCount();

  @override
  int pingProfile(String id) => rust.pingProfile(id: id);

  @override
  Stream<rust.ProgressEvent> progressStream(int count) =>
      rust.progressStream(count: count);

  @override
  rust.UiEventAck echoEvent(int seq, String kind) =>
      rust.echoUiEvent(seq: BigInt.from(seq), kind: kind);

  @override
  Future<int> simulateBlocking(int ms) async {
    final elapsed = await rust.simulateBlocking(ms: BigInt.from(ms));
    return elapsed.toInt();
  }
}

/// Loads the FRB dynamic library; isolated so tests can avoid touching it.
class RustBridgeInit {
  static Future<void> Function()? _delegate;

  static void configure(Future<void> Function() delegate) =>
      _delegate = delegate;

  static Future<void> init() async {
    final delegate = _delegate;
    if (delegate == null) {
      throw StateError('Rust bridge init delegate is not configured');
    }
    await delegate();
  }
}

/// Deterministic Dart-side generator used only by tests.
class SyntheticBridgePort implements BridgePort {
  SyntheticBridgePort({this.count = 10000});

  final int count;
  List<ProfileSummary>? _rows;

  @override
  Future<void> init() async {}

  @override
  List<ProfileSummary> generate(int count) {
    _rows = List<ProfileSummary>.generate(count, _build);
    return _rows!;
  }

  @override
  int rustProfileCount() => _rows?.length ?? 0;

  @override
  int pingProfile(String id) {
    final rows = _rows ?? const <ProfileSummary>[];
    final row = rows.firstWhere((r) => r.id == id, orElse: () => _build(0));
    return row.delay < 0 ? 0 : row.delay;
  }

  @override
  Stream<rust.ProgressEvent> progressStream(int count) async* {
    var done = 0;
    var seq = 0;
    while (done < count) {
      final step = done + 1000 <= count ? 1000 : count - done;
      done += step;
      seq++;
      yield rust.ProgressEvent(seq: seq, done: done, total: count);
    }
  }

  @override
  rust.UiEventAck echoEvent(int seq, String kind) => rust.UiEventAck(
    seq: BigInt.from(seq),
    kind: kind,
    rustProfileCount: rustProfileCount(),
  );

  @override
  Future<int> simulateBlocking(int ms) async => ms;

  ProfileSummary _build(int index) {
    final h = (index * 2654435761) & 0x7fffffff;
    const blocks = <String>['192.0.2', '198.51.100', '203.0.113'];
    const domains = <String>['example.com', 'example.org', 'example.net'];
    final block = blocks[h % blocks.length];
    final octet = 1 + (h ~/ 7) % 254;
    final isDomain = h.isEven;
    final address = isDomain
        ? 'node${index.toString().padLeft(5, '0')}.${domains[h % domains.length]}'
        : '$block.$octet';
    final delay = h % 11 == 0 ? -1 : h % 400;
    final gb = BigInt.from(1073741824);
    return ProfileSummary(
      id: 'syn-${index.toString().padLeft(6, '0')}',
      configType: ConfigType.values[h % ConfigType.values.length],
      remarks: 'Synthetic-${index.toString().padLeft(5, '0')}',
      address: address,
      port: 10000 + (h % 50000),
      network: const ['tcp', 'ws', 'grpc', 'http'][h % 4],
      streamSecurity: const ['none', 'tls', 'reality'][h % 3],
      subRemarks: 'sub-${(index ~/ 100).toString().padLeft(3, '0')}',
      delay: delay,
      speed: delay < 0 ? '-' : '${((h % 5000) / 100).toStringAsFixed(2)} MB/s',
      todayUp: gb * BigInt.from(h % 50) ~/ BigInt.from(100),
      ipInfo: h % 5 == 0 ? '-' : '$block.$octet',
      todayDown: gb * BigInt.from(h % 500) ~/ BigInt.from(10),
      totalUp: gb * BigInt.from(h % 400),
      totalDown: gb * BigInt.from(h % 4000),
      coreType: CoreType.values[h % CoreType.values.length],
    );
  }
}

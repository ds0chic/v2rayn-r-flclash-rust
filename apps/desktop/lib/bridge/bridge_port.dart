import 'dart:convert';

import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/dns.dart' as dns;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/groups.dart' as groups;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as monitor;
import 'package:v2rayn_desktop/bridge/api/profiles.dart' as rust;
import 'package:v2rayn_desktop/bridge/api/routing.dart' as routing;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/api/speedtest.dart' as speedtest;
import 'package:v2rayn_desktop/bridge/api/subs.dart' as subs;
import 'package:v2rayn_desktop/bridge/api/t16.dart' as t16;
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

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

  // -- T06a profile repository surface -----------------------------------

  /// Rows for the node table. Synthetic data in tests; real query in
  /// production.
  List<ProfileSummary> fetchSummaries(int count);

  /// Every profile currently stored, used by the editor and batch actions.
  ///
  /// Implementations must follow the store's real cursor across bounded pages;
  /// a store larger than one page is never truncated (D09).
  List<c.ProfileDto> queryAllProfiles();

  /// One bounded page of node-table summaries through the real Rust cursor.
  /// [pageSize] must stay bounded; the store's own `nextCursor` is surfaced
  /// unchanged (no fabricated end-of-list).
  ProfileSummaryPage querySummaryPage({
    int cursor = 0,
    int pageSize = kProfileQueryPageSize,
    String? text,
    String? subid,
  });

  /// Every node-table summary assembled by following [querySummaryPage]'s real
  /// cursor to exhaustion, so a store larger than one page is never truncated.
  List<ProfileSummary> queryAllSummaries({String? text, String? subid});

  /// One structural read of the profile store (ordered base rows + full DTOs
  /// from the same pass). Used by reload so it does not query the table twice.
  ProfileSnapshot fetchProfileSnapshot(
    int count, {
    String? text,
    String? subid,
  });

  /// Re-join the live speedtest result + statistics overlay onto [base] without
  /// re-reading the profile table. This is the speedtest poll path, so the
  /// 150 ms cadence never re-runs the full summary query (D08).
  List<ProfileSummary> applyLiveOverlay(List<ProfileSummary> base);

  /// One full profile by stable id, if present.
  c.ProfileDto? getProfile(String id);

  /// Current desired revision for optimistic saves.
  int profileRevision();

  c.SaveProfileResult saveProfile(c.ProfileDto draft, int expectedRevision);

  /// Import-pipeline save: empty remarks/address/port are valid (FIX-04).
  c.SaveProfileResult saveImportedProfile(
    c.ProfileDto draft,
    int expectedRevision,
  );

  c.DeleteProfilesResult deleteProfiles(List<String> ids);

  c.CopyProfilesResult copyProfiles(List<String> ids);

  c.SaveProfileResult setProfileRemarks(String id, String remarks);

  c.SimpleResult setActiveProfile(String? id);

  String? getActiveProfile();

  /// Point the engine at an explicit data directory (tests/portable). No-op
  /// once the engine is live.
  c.SimpleResult initEngine(String? dataDir);

  /// Copy a user-selected custom/outbound config file into the data
  /// directory's `config/` folder and return the stored file name for
  /// `Profile.address` (upstream `AddCustomServer` /
  /// `AddCustomOutboundServer` browse step, FIX-03B).
  c.CustomFileResult customImportFile(String sourcePath);

  /// The resolved data directory currently in use (FIX-03B, for resolving a
  /// stored config file name back to its full path).
  String dataDir();

  // -- T09 subscription + import/export surface --------------------------

  c.SubsPageDto listSubItems();

  c.SubItemDto? getSubItem(String id);

  c.SubItemDtoResult saveSubItem(c.SubItemDto item);

  c.DeleteSubsResult deleteSubItems(List<String> ids);

  c.SubItemDtoResult setSubEnabled(String id, bool enabled);

  c.SimpleResult reorderSubItems(List<String> ids);

  c.SimpleResult validateSubItem(c.SubItemDto item);

  void setLocalProxyPort(int? port);

  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  );

  Future<c.SubUpdateResult> updateSubscription(String subId, bool viaProxy);

  c.SimpleResult startSubScheduler();

  c.SimpleResult stopSubScheduler();

  bool subSchedulerRunning();

  c.JobDto? jobView(String jobId);

  /// Idempotent cancellation of an in-flight job (F-SUB-003).
  c.CancelResult cancelJob(String jobId);

  Future<c.ImportResult> importFromText(
    String text, {
    String? subid,
    bool deduplicate = true,
  });

  c.UriParseResult parseShareUri(String line);

  Future<c.ShareExportResult> exportProfiles(List<String> ids, String kind);

  c.SimpleResult writeExportFile(String path, String text);

  /// Full client config text for [id] (upstream `Export2ClientConfigAsync` /
  /// `Export2ClientConfigResult`). Reuses the engine `build_codegen_input` +
  /// `generate` path; an unknown node or codegen failure is a structured error,
  /// never a fake config.
  c.ShareExportResult exportClientConfigText(String id);

  // -- T10 group / custom / template surface -----------------------------

  groups.TemplatesPageDto listTemplates();

  groups.FullConfigTemplateDto? getTemplate(CoreType core);

  groups.TemplateDtoResult saveTemplate(groups.FullConfigTemplateDto item);

  c.SimpleResult deleteTemplate(String id);

  c.ProfilePageDto groupChildren(String indexId);

  c.SaveProfileResult genGroupAll(String subId);

  groups.GroupGenResult genGroupRegion(String subId);

  // -- T12a settings surface ---------------------------------------------

  /// The full normalised `guiNConfig.json` tree, its revision counters and the
  /// canonical JSON the UI edits.
  settings.SettingsLoadDto getSettings();

  /// Whole-tree settings save (optimistic revision).
  settings.SaveSettingsResult saveSettingsJson(
    String settingsJson,
    int expectedRevision,
  );

  /// Replace one top-level group by canonical JSON patch.
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String patchJson,
    int expectedRevision,
  );

  int settingsRevision();

  // -- T11 routing / DNS surface -----------------------------------------

  routing.RoutingsPageDto listRoutings();

  routing.RoutingDtoResult getRouting(String id);

  routing.RoutingDtoResult saveRouting(routing.RoutingProfileDto draft);

  c.SimpleResult deleteRouting(String id);

  c.SimpleResult setDefaultRouting(String id);

  routing.RoutingRulesPageDto listRoutingRules(String routingId);

  routing.RoutingDtoResult saveRoutingRules(
    String routingId,
    List<routing.RoutingRuleDto> rules,
  );

  routing.RoutingDtoResult moveRoutingRule(
    String routingId,
    int index,
    int direction,
  );

  routing.RoutingRulesTextResult importRoutingRules(
    String routingId,
    String text,
    bool replace,
  );

  routing.RoutingRulesTextResult exportRoutingRules(
    String routingId,
    List<String> ids,
  );

  routing.RuleModeResult getRuleMode();

  c.SimpleResult setRuleMode(String mode);

  dns.DnsPageDto listDns();

  dns.DnsDtoResult saveDns(dns.DnsProfileDto draft);

  dns.DnsDtoResult importDefaultDns(CoreType core);

  dns.SimpleDnsDtoResult loadSimpleDns();

  dns.SimpleDnsDtoResult saveSimpleDns(dns.SimpleDnsDto draft, int revision);

  dns.RegionalPresetResult applyRegionalPreset(String preset);

  String defaultDnsText(String kind);

  // -- T15b speedtest surface --------------------------------------------

  /// Which test actions the backend can really perform (UDP stays disabled).
  speedtest.SpeedTestSupportDto speedTestSupport();

  /// Apply the effective `SpeedTestItem` settings before starting a job.
  c.SimpleResult configureSpeedTest({
    required int pageSize,
    required int mixedConcurrency,
    required int timeoutSecs,
    required String speedTestUrl,
    required String speedPingTestUrl,
    String? ipapiUrl,
    String? udpTestTarget,
    required int delayIntervalSecs,
  });

  /// Start a speedtest job over the selected ids (empty = all nodes).
  speedtest.SpeedTestStartDto startSpeedTest(int kind, List<String> indexIds);

  /// Idempotent cancel of a running job.
  c.SimpleResult cancelSpeedTest(String jobId);

  /// Current `ProfileExItem` result rows used to overlay Delay/Speed/IpInfo.
  List<speedtest.SpeedTestResultDto> speedTestResults();

  /// `RemoveInvalidServerResult`: delete rows whose delay failed.
  int removeInvalidResults();

  /// Group-scoped `RemoveInvalidServerResult`: prune failed result rows that
  /// belong to [subid]'s stored profiles, leaving other groups untouched.
  int removeInvalidResultsForGroup(String subid);

  /// Number of speedtest jobs still running (drives live UI refresh).
  int speedTestActiveJobs();

  /// Persist the node table's display order (PR-15): rewrite the upstream
  /// `ProfileExItem.Sort` field (`(i + 1) * 10`) for [orderedIds] and flush it
  /// to SQLite, mirroring `ConfigHandler.MoveServer`/`SortServers`. A reopened
  /// process reads the same `Sort` back. An empty list is a no-op.
  c.SimpleResult applyProfileOrder(List<String> orderedIds);

  // -- T16 backup / WebDAV / update surface ------------------------------
  //
  // D10/R4-10: the backup bundle paths copy files, hash the DB and swap the
  // database lifecycle. They are async so that work runs off the UI isolate;
  // `Future.sync` keeps this compilable both before and after the FRB
  // regenerator flips the Rust source from `#[frb(sync)]` to async.

  Future<c.BackupResultDto> t16BackupLocal(String destRoot);

  Future<c.BackupListDto> t16BackupList(String parent);

  Future<c.VerificationDto> t16BackupVerify(String bundleDir);

  Future<c.RestoreResultDto> t16BackupRestore(String bundleDir);

  Future<c.RecognitionDto> t16BackupRecognize(String path);

  Future<c.ImportSummaryDto> t16BackupImportUpstream(String path);

  c.WebDavConfigResultDto t16WebdavConfigGet();

  c.WebDavConfigResultDto t16WebdavConfigSave(
    c.WebDavConfigDto cfg,
    int expectedRevision,
  );

  Future<c.WebDavCheckDto> t16WebdavCheck(c.WebDavConfigDto cfg);

  Future<c.WebDavListDto> t16WebdavList(c.WebDavConfigDto cfg);

  Future<c.WebDavOpDto> t16WebdavBackup(c.WebDavConfigDto cfg);

  Future<c.RestoreResultDto> t16WebdavRestore(c.WebDavConfigDto cfg);

  List<c.UpdateTargetDto> t16UpdateTargets();

  Future<c.UpdateReportDto> t16CheckUpdates(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  );

  Future<c.ApplyCoreResultDto> t16ApplyCoreUpdate(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  );

  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec();

  c.CleanupResultDto t16CleanupLogsTmp();

  c.SimpleResult t16OpenConfigDir();

  c.CoreVersionsDto t16GetCoreVersions();
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

  @override
  List<ProfileSummary> fetchSummaries(int count) {
    final snapshot = fetchProfileSnapshot(count);
    return applyLiveOverlay(snapshot.summaries);
  }

  @override
  ProfileSummaryPage querySummaryPage({
    int cursor = 0,
    int pageSize = kProfileQueryPageSize,
    String? text,
    String? subid,
  }) {
    final page = engine.queryProfiles(
      filter: c.ProfileFilterDto(
        text: text,
        configTypes: const [],
        subid: subid,
      ),
      sort: c.ProfileSortDto.indexId,
      cursor: BigInt.from(cursor),
      pageSize: pageSize,
    );
    return ProfileSummaryPage(
      items: page.items.map(dtoToSummary).toList(growable: false),
      total: page.total.toInt(),
      nextCursor: page.nextCursor?.toInt(),
    );
  }

  @override
  List<ProfileSummary> queryAllSummaries({String? text, String? subid}) {
    final out = <ProfileSummary>[];
    var cursor = 0;
    for (;;) {
      final page = querySummaryPage(cursor: cursor, text: text, subid: subid);
      out.addAll(page.items);
      final next = page.nextCursor;
      if (next == null || next <= cursor || page.items.isEmpty) break;
      cursor = next;
    }
    return out;
  }

  @override
  ProfileSnapshot fetchProfileSnapshot(
    int count, {
    String? text,
    String? subid,
  }) {
    // One cursor-following read of the stored profiles; the persisted display
    // order (`ProfileExItem.Sort`) is applied from the same pass, so a reload
    // never falls back to index-id order and never reads the table twice.
    final profiles = queryAllProfiles();
    final results = speedTestResults();
    final summaries = orderByPersistedSort(
      profiles.map(dtoToSummary).toList(),
      results.map((r) => r.indexId).toList(),
    );
    return ProfileSnapshot(summaries: summaries, profiles: profiles);
  }

  @override
  List<ProfileSummary> applyLiveOverlay(List<ProfileSummary> base) {
    // Join the live monitor `ServerStatItem` rows onto the table
    // (upstream `ProfilesViewModel.GetProfileItemsEx`: join on IndexId). The
    // snapshot is a read-only view of the background statistics collection, so
    // reading it never pauses collection or depends on page visibility.
    // Upstream shows no traffic at all when statistics are disabled, so the
    // overlay is skipped in that case instead of fabricating zero rows.
    final results = speedTestResults();
    final stats = monitor.statsSnapshot();
    final rows = stats.enabled
        ? applyNodeStatsOverlay(base, stats.nodes.map(nodeStatFromDto))
        : base;
    return applySpeedTestOverlay(rows, results);
  }

  @override
  List<c.ProfileDto> queryAllProfiles() {
    final out = <c.ProfileDto>[];
    var cursor = 0;
    for (;;) {
      final page = engine.queryProfiles(
        filter: const c.ProfileFilterDto(
          text: null,
          configTypes: [],
          subid: null,
        ),
        sort: c.ProfileSortDto.indexId,
        cursor: BigInt.from(cursor),
        pageSize: kProfileQueryPageSize,
      );
      out.addAll(page.items);
      final next = page.nextCursor?.toInt();
      if (next == null || next <= cursor || page.items.isEmpty) break;
      cursor = next;
    }
    return out;
  }

  @override
  c.ProfileDto? getProfile(String id) => engine.getProfile(indexId: id);

  @override
  int profileRevision() => engine.profileRevision().toInt();

  @override
  c.SaveProfileResult saveProfile(c.ProfileDto draft, int expectedRevision) =>
      engine.saveProfile(
        draft: draft,
        expectedRevision: BigInt.from(expectedRevision),
      );

  @override
  c.SaveProfileResult saveImportedProfile(
    c.ProfileDto draft,
    int expectedRevision,
  ) => engine.saveImportedProfile(
    draft: draft,
    expectedRevision: BigInt.from(expectedRevision),
  );

  @override
  c.DeleteProfilesResult deleteProfiles(List<String> ids) =>
      engine.deleteProfiles(ids: ids);

  @override
  c.CopyProfilesResult copyProfiles(List<String> ids) =>
      engine.copyProfiles(ids: ids);

  @override
  c.SaveProfileResult setProfileRemarks(String id, String remarks) =>
      engine.setProfileRemarks(indexId: id, remarks: remarks);

  @override
  c.SimpleResult setActiveProfile(String? id) =>
      engine.setActiveProfile(indexId: id);

  @override
  String? getActiveProfile() => engine.getActiveProfile();

  @override
  c.SimpleResult initEngine(String? dataDir) =>
      engine.initEngine(dataDir: dataDir);

  @override
  c.CustomFileResult customImportFile(String sourcePath) =>
      engine.customImportFile(sourcePath: sourcePath);

  @override
  String dataDir() => engine.dataDir();

  @override
  c.SubsPageDto listSubItems() => subs.listSubItems();

  @override
  c.SubItemDto? getSubItem(String id) => subs.getSubItem(id: id);

  @override
  c.SubItemDtoResult saveSubItem(c.SubItemDto item) =>
      subs.saveSubItem(item: item);

  @override
  c.DeleteSubsResult deleteSubItems(List<String> ids) =>
      subs.deleteSubItems(ids: ids);

  @override
  c.SubItemDtoResult setSubEnabled(String id, bool enabled) =>
      subs.setSubEnabled(id: id, enabled: enabled);

  @override
  c.SimpleResult reorderSubItems(List<String> ids) =>
      subs.reorderSubItems(ids: ids);

  @override
  c.SimpleResult validateSubItem(c.SubItemDto item) =>
      subs.validateSubItem(item: item);

  @override
  void setLocalProxyPort(int? port) => subs.setLocalProxyPort(port: port);

  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) => subs.updateSubscriptions(subIds: subIds, viaProxy: viaProxy);

  @override
  Future<c.SubUpdateResult> updateSubscription(String subId, bool viaProxy) =>
      subs.updateSubscription(subId: subId, viaProxy: viaProxy);

  @override
  c.SimpleResult startSubScheduler() => subs.startSubScheduler();

  @override
  c.SimpleResult stopSubScheduler() => subs.stopSubScheduler();

  @override
  bool subSchedulerRunning() => subs.subSchedulerRunning();

  @override
  c.JobDto? jobView(String jobId) => subs.jobView(jobId: jobId);

  @override
  c.CancelResult cancelJob(String jobId) => engine.cancelJob(jobId: jobId);

  @override
  Future<c.ImportResult> importFromText(
    String text, {
    String? subid,
    bool deduplicate = true,
  }) => subs.importFromText(text: text, subid: subid, deduplicate: deduplicate);

  @override
  c.UriParseResult parseShareUri(String line) => subs.parseShareUri(line: line);

  @override
  Future<c.ShareExportResult> exportProfiles(List<String> ids, String kind) =>
      subs.exportProfiles(ids: ids, kind: kind);

  @override
  c.SimpleResult writeExportFile(String path, String text) =>
      subs.writeExportFile(path: path, text: text);

  @override
  c.ShareExportResult exportClientConfigText(String id) {
    final result = speedtest.exportClientConfig(indexId: id);
    return c.ShareExportResult(
      ok: result.ok,
      text: result.text,
      count: result.ok ? 1 : 0,
      error: result.error,
    );
  }

  @override
  groups.TemplatesPageDto listTemplates() => groups.listTemplates();

  @override
  groups.FullConfigTemplateDto? getTemplate(CoreType core) =>
      groups.getTemplate(core: core);

  @override
  groups.TemplateDtoResult saveTemplate(groups.FullConfigTemplateDto item) =>
      groups.saveTemplate(item: item);

  @override
  c.SimpleResult deleteTemplate(String id) => groups.deleteTemplate(id: id);

  @override
  c.ProfilePageDto groupChildren(String indexId) =>
      groups.groupChildren(indexId: indexId);

  @override
  c.SaveProfileResult genGroupAll(String subId) =>
      groups.genGroupAll(subId: subId);

  @override
  groups.GroupGenResult genGroupRegion(String subId) =>
      groups.genGroupRegion(subId: subId);

  @override
  settings.SettingsLoadDto getSettings() => settings.getSettings();

  @override
  settings.SaveSettingsResult saveSettingsJson(
    String settingsJson,
    int expectedRevision,
  ) => settings.saveSettingsJson(
    settingsJson: settingsJson,
    expectedRevision: BigInt.from(expectedRevision),
  );

  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String patchJson,
    int expectedRevision,
  ) => settings.saveSettingsGroup(
    group: group,
    patchJson: patchJson,
    expectedRevision: BigInt.from(expectedRevision),
  );

  @override
  int settingsRevision() => settings.settingsRevision().toInt();

  // -- T11 routing / DNS (FRB) -------------------------------------------

  @override
  routing.RoutingsPageDto listRoutings() => routing.listRoutings();

  @override
  routing.RoutingDtoResult getRouting(String id) => routing.getRouting(id: id);

  @override
  routing.RoutingDtoResult saveRouting(routing.RoutingProfileDto draft) =>
      routing.saveRouting(draft: draft);

  @override
  c.SimpleResult deleteRouting(String id) => routing.deleteRouting(id: id);

  @override
  c.SimpleResult setDefaultRouting(String id) =>
      routing.setDefaultRouting(id: id);

  @override
  routing.RoutingRulesPageDto listRoutingRules(String routingId) =>
      routing.listRoutingRules(routingId: routingId);

  @override
  routing.RoutingDtoResult saveRoutingRules(
    String routingId,
    List<routing.RoutingRuleDto> rules,
  ) => routing.saveRoutingRules(routingId: routingId, rules: rules);

  @override
  routing.RoutingDtoResult moveRoutingRule(
    String routingId,
    int index,
    int direction,
  ) => routing.moveRoutingRule(
    routingId: routingId,
    index: index,
    direction: direction,
  );

  @override
  routing.RoutingRulesTextResult importRoutingRules(
    String routingId,
    String text,
    bool replace,
  ) => routing.importRoutingRules(
    routingId: routingId,
    text: text,
    replace: replace,
  );

  @override
  routing.RoutingRulesTextResult exportRoutingRules(
    String routingId,
    List<String> ids,
  ) => routing.exportRoutingRules(routingId: routingId, ids: ids);

  @override
  routing.RuleModeResult getRuleMode() => routing.getRuleMode();

  @override
  c.SimpleResult setRuleMode(String mode) => routing.setRuleMode(mode: mode);

  @override
  dns.DnsPageDto listDns() => dns.listDns();

  @override
  dns.DnsDtoResult saveDns(dns.DnsProfileDto draft) =>
      dns.saveDns(draft: draft);

  @override
  dns.DnsDtoResult importDefaultDns(CoreType core) =>
      dns.importDefaultDns(core: core);

  @override
  dns.SimpleDnsDtoResult loadSimpleDns() => dns.loadSimpleDns();

  @override
  dns.SimpleDnsDtoResult saveSimpleDns(dns.SimpleDnsDto draft, int revision) =>
      dns.saveSimpleDns(draft: draft, expectedRevision: BigInt.from(revision));

  @override
  dns.RegionalPresetResult applyRegionalPreset(String preset) =>
      dns.applyRegionalPreset(preset: preset);

  @override
  String defaultDnsText(String kind) => dns.defaultDnsText(kind: kind);

  @override
  speedtest.SpeedTestSupportDto speedTestSupport() =>
      speedtest.speedtestSupported();

  @override
  c.SimpleResult configureSpeedTest({
    required int pageSize,
    required int mixedConcurrency,
    required int timeoutSecs,
    required String speedTestUrl,
    required String speedPingTestUrl,
    String? ipapiUrl,
    String? udpTestTarget,
    required int delayIntervalSecs,
  }) => speedtest.speedtestConfigure(
    pageSize: pageSize,
    mixedConcurrency: mixedConcurrency,
    timeoutSecs: timeoutSecs,
    speedTestUrl: speedTestUrl,
    speedPingTestUrl: speedPingTestUrl,
    ipapiUrl: ipapiUrl,
    udpTestTarget: udpTestTarget,
    delayIntervalSecs: delayIntervalSecs,
  );

  @override
  speedtest.SpeedTestStartDto startSpeedTest(int kind, List<String> indexIds) =>
      speedtest.speedtestStart(kind: kind, indexIds: indexIds);

  @override
  c.SimpleResult cancelSpeedTest(String jobId) =>
      speedtest.speedtestCancel(jobId: jobId);

  @override
  List<speedtest.SpeedTestResultDto> speedTestResults() =>
      speedtest.speedtestResults();

  @override
  int removeInvalidResults() => speedtest.speedtestRemoveInvalid();

  @override
  int removeInvalidResultsForGroup(String subid) =>
      speedtest.speedtestRemoveInvalidGroup(subid: subid);

  @override
  int speedTestActiveJobs() => speedtest.speedtestActiveJobs();

  @override
  c.SimpleResult applyProfileOrder(List<String> orderedIds) =>
      speedtest.speedtestApplyProfileOrder(orderedIds: orderedIds);

  // -- T16 backup / WebDAV / update (FRB) --------------------------------

  @override
  Future<c.BackupResultDto> t16BackupLocal(String destRoot) =>
      Future<c.BackupResultDto>.sync(
        () => t16.t16BackupLocal(destRoot: destRoot),
      );

  @override
  Future<c.BackupListDto> t16BackupList(String parent) =>
      Future<c.BackupListDto>.sync(() => t16.t16BackupList(parent: parent));

  @override
  Future<c.VerificationDto> t16BackupVerify(String bundleDir) =>
      Future<c.VerificationDto>.sync(
        () => t16.t16BackupVerify(bundleDir: bundleDir),
      );

  @override
  Future<c.RestoreResultDto> t16BackupRestore(String bundleDir) =>
      Future<c.RestoreResultDto>.sync(
        () => t16.t16BackupRestore(bundleDir: bundleDir),
      );

  @override
  Future<c.RecognitionDto> t16BackupRecognize(String path) =>
      Future<c.RecognitionDto>.sync(() => t16.t16BackupRecognize(path: path));

  @override
  Future<c.ImportSummaryDto> t16BackupImportUpstream(String path) =>
      Future<c.ImportSummaryDto>.sync(
        () => t16.t16BackupImportUpstream(path: path),
      );

  @override
  c.WebDavConfigResultDto t16WebdavConfigGet() => t16.t16WebdavConfigGet();

  @override
  c.WebDavConfigResultDto t16WebdavConfigSave(
    c.WebDavConfigDto cfg,
    int expectedRevision,
  ) => t16.t16WebdavConfigSave(
    cfg: cfg,
    expectedRevision: BigInt.from(expectedRevision),
  );

  @override
  Future<c.WebDavCheckDto> t16WebdavCheck(c.WebDavConfigDto cfg) =>
      t16.t16WebdavCheck(cfg: cfg);

  @override
  Future<c.WebDavListDto> t16WebdavList(c.WebDavConfigDto cfg) =>
      t16.t16WebdavList(cfg: cfg);

  @override
  Future<c.WebDavOpDto> t16WebdavBackup(c.WebDavConfigDto cfg) =>
      t16.t16WebdavBackup(cfg: cfg);

  @override
  Future<c.RestoreResultDto> t16WebdavRestore(c.WebDavConfigDto cfg) =>
      t16.t16WebdavRestore(cfg: cfg);

  @override
  List<c.UpdateTargetDto> t16UpdateTargets() => t16.t16UpdateTargets();

  @override
  Future<c.UpdateReportDto> t16CheckUpdates(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  ) => t16.t16CheckUpdates(
    cores: cores,
    prerelease: prerelease,
    viaProxy: viaProxy,
  );

  @override
  Future<c.ApplyCoreResultDto> t16ApplyCoreUpdate(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  ) => t16.t16ApplyCoreUpdate(
    cores: cores,
    prerelease: prerelease,
    viaProxy: viaProxy,
  );

  @override
  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec() =>
      t16.t16ApplyAppUpdateSpec();

  @override
  c.CleanupResultDto t16CleanupLogsTmp() => t16.t16CleanupLogsTmp();

  @override
  c.SimpleResult t16OpenConfigDir() => t16.t16OpenConfigDir();

  @override
  c.CoreVersionsDto t16GetCoreVersions() => t16.t16GetCoreVersions();
}

/// Overlay `ProfileExItem` results onto the node-table summaries. A missing
/// row leaves the "unknown" defaults untouched (never a fake value).
///
/// A result row with `delay < 0` means the node was really tested and failed;
/// it is mapped to [profileDelayTestFailed] so the table can tell "tested and
/// failed" apart from "never tested" (both use `delay < 0` on the wire).
List<ProfileSummary> applySpeedTestOverlay(
  List<ProfileSummary> rows,
  List<speedtest.SpeedTestResultDto> results,
) {
  if (results.isEmpty || rows.isEmpty) return rows;
  final byId = <String, speedtest.SpeedTestResultDto>{
    for (final r in results) r.indexId: r,
  };
  return rows.map((row) {
    final result = byId[row.id];
    if (result == null) return row;
    final hasDelay = result.delay != 0;
    final displayDelay = result.delay < 0
        ? profileDelayTestFailed
        : result.delay;
    final speedText = result.speed > 0
        ? '${result.speed.toStringAsFixed(1)} MB/s'
        : row.speed;
    return ProfileSummary(
      id: row.id,
      configType: row.configType,
      remarks: row.remarks,
      address: row.address,
      port: row.port,
      network: row.network,
      streamSecurity: row.streamSecurity,
      subRemarks: row.subRemarks,
      delay: hasDelay ? displayDelay : row.delay,
      speed: speedText,
      todayUp: row.todayUp,
      ipInfo: result.ipInfo.isNotEmpty ? result.ipInfo : row.ipInfo,
      todayDown: row.todayDown,
      totalUp: row.totalUp,
      totalDown: row.totalDown,
      coreType: row.coreType,
    );
  }).toList();
}

/// UI-only delay sentinel: the node was tested and the test failed. Distinct
/// from the wire `-1` (which also means "unknown" for an untested row).
const int profileDelayTestFailed = -2;

/// Join node-picker candidates (`ProfileDto`) with the live `ProfileExItem`
/// speedtest results and the subscription remarks, producing table-shaped
/// [ProfileSummary] rows. The picker's Delay/Speed/SubRemarks columns and its
/// default `Sort` order then follow the node table instead of showing `-` and
/// the raw `subid` (upstream `ProfilesSelectViewModel.GetProfileItemsEx`
/// joins `ProfileItem` with `ProfileExItem`; R3-PROF-08). A candidate with no
/// matching result keeps the "unknown" defaults.
List<ProfileSummary> joinPickerRows(
  List<c.ProfileDto> candidates,
  List<speedtest.SpeedTestResultDto> results,
  List<c.SubItemDto> subItems,
) {
  final remarksById = <String, String>{
    for (final s in subItems) s.id: (s.remarks.isEmpty ? s.id : s.remarks),
  };
  final rows = <ProfileSummary>[
    for (final dto in candidates)
      ProfileSummary(
        id: dto.indexId,
        configType: dto.configType,
        remarks: dto.remarks,
        address: dto.address,
        port: dto.port,
        network: dto.network,
        streamSecurity: dto.security.streamSecurity ?? '',
        // Subscription column shows `SubRemarks`, falling back to `subid`.
        subRemarks: remarksById[dto.subid] ?? dto.subid,
        delay: -1,
        speed: '-',
        todayUp: BigInt.zero,
        ipInfo: '-',
        todayDown: BigInt.zero,
        totalUp: BigInt.zero,
        totalDown: BigInt.zero,
        coreType: dto.coreType ?? CoreType.xray,
      ),
  ];
  return applySpeedTestOverlay(rows, results);
}

/// Per-node `ServerStatItem` counters consumed by the node table, normalized to
/// [BigInt] so the same value shape works on native (`BigInt`) and web (`int`).
class NodeStat {
  const NodeStat({
    required this.indexId,
    required this.todayUp,
    required this.todayDown,
    required this.totalUp,
    required this.totalDown,
  });

  final String indexId;
  final BigInt todayUp;
  final BigInt todayDown;
  final BigInt totalUp;
  final BigInt totalDown;
}

/// Convert a monitor `NodeTrafficDto` into the table-facing [NodeStat].
NodeStat nodeStatFromDto(monitor.NodeTrafficDto dto) => NodeStat(
  indexId: dto.indexId,
  todayUp: _statBytes(dto.todayUp),
  todayDown: _statBytes(dto.todayDown),
  totalUp: _statBytes(dto.totalUp),
  totalDown: _statBytes(dto.totalDown),
);

/// Join per-node statistics onto the node table by `IndexId` (upstream
/// `ProfilesViewModel.GetProfileItemsEx:404-433`). Today/cumulative up/down are
/// raw byte counters from the Rust `StatsService`; a row without a matching
/// stat keeps the summary's "unknown" zero defaults (never a fabricated value).
/// Because the join is by the current row id, a deleted node or a switched
/// database cannot inherit another row's counters.
List<ProfileSummary> applyNodeStatsOverlay(
  List<ProfileSummary> rows,
  Iterable<NodeStat> nodes,
) {
  final byId = <String, NodeStat>{for (final n in nodes) n.indexId: n};
  if (byId.isEmpty || rows.isEmpty) return rows;
  return rows.map((row) {
    final stat = byId[row.id];
    if (stat == null) return row;
    return ProfileSummary(
      id: row.id,
      configType: row.configType,
      remarks: row.remarks,
      address: row.address,
      port: row.port,
      network: row.network,
      streamSecurity: row.streamSecurity,
      subRemarks: row.subRemarks,
      delay: row.delay,
      speed: row.speed,
      todayUp: stat.todayUp,
      ipInfo: row.ipInfo,
      todayDown: stat.todayDown,
      totalUp: stat.totalUp,
      totalDown: stat.totalDown,
      coreType: row.coreType,
    );
  }).toList();
}

BigInt _statBytes(Object? value) {
  if (value is BigInt) return value.isNegative ? BigInt.zero : value;
  if (value is int) return value > 0 ? BigInt.from(value) : BigInt.zero;
  if (value is num) {
    final v = value.toInt();
    return v > 0 ? BigInt.from(v) : BigInt.zero;
  }
  return BigInt.zero;
}

/// Map a stored profile DTO onto the node-table summary shape. Traffic/delay
/// fields stay at their "unknown" defaults (never fabricated); the node table
/// fills the four traffic columns from the monitor via [applyNodeStatsOverlay].
ProfileSummary dtoToSummary(c.ProfileDto dto) => ProfileSummary(
  id: dto.indexId,
  configType: dto.configType,
  remarks: dto.remarks,
  address: dto.address,
  port: dto.port,
  network: dto.network,
  streamSecurity: dto.security.streamSecurity ?? '',
  subRemarks: dto.subid,
  delay: -1,
  speed: '-',
  todayUp: BigInt.zero,
  ipInfo: '-',
  todayDown: BigInt.zero,
  totalUp: BigInt.zero,
  totalDown: BigInt.zero,
  coreType: dto.coreType ?? CoreType.xray,
);

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
///
/// UI-state assertions only: synthetic successes (`added: 3`) never traverse
/// the Rust pipeline. Successful subscription replacement is evidenced
/// solely by the Rust `subs_pipeline` tests; widget tests must not be cited
/// as replacement evidence. Disk-writing operations (`writeExportFile`)
/// report a structured "not wired" failure instead of faking success.
class SyntheticBridgePort implements BridgePort {
  SyntheticBridgePort({this.count = 10000});

  final int count;
  List<ProfileSummary>? _rows;
  int _summaryCount = 0;
  final List<c.ProfileDto> _profiles = <c.ProfileDto>[];
  bool _seeded = false;
  int _revision = 0;
  String? _active;
  int _newId = 0;

  /// T15b test seams: recorded calls and an in-memory result overlay.
  final Map<String, speedtest.SpeedTestResultDto> _speedResults = {};
  final List<Map<String, Object?>> speedTestCalls = [];
  final List<String> cancelledTestJobs = [];
  String? lastSpeedTestJobId;
  String? lastSpeedTestConfig;
  int removeInvalidCalls = 0;

  /// Persisted display order stand-in for `ProfileExItem.Sort`; [fetchSummaries]
  /// reads it back so the synthetic bridge models the read chain.
  final List<String> _persistedOrder = <String>[];

  /// Controlled monitor `ServerStatItem` fixture keyed by id. The real
  /// [FrbBridgePort] reads the same shape from `monitor.statsSnapshot()`;
  /// seeding this models that read chain without fabricating traffic values in
  /// production. [statsEnabled] mirrors the upstream `EnableStatistics` gate.
  final Map<String, NodeStat> _nodeStats = <String, NodeStat>{};
  bool statsEnabled = false;

  /// Publish one controlled statistics row (today/total up/down, raw bytes).
  void seedNodeStat(
    String id, {
    int todayUp = 0,
    int todayDown = 0,
    int totalUp = 0,
    int totalDown = 0,
  }) {
    _nodeStats[id] = NodeStat(
      indexId: id,
      todayUp: BigInt.from(todayUp),
      todayDown: BigInt.from(todayDown),
      totalUp: BigInt.from(totalUp),
      totalDown: BigInt.from(totalDown),
    );
  }

  /// Drop one controlled statistics row, as a node deletion must remove its
  /// mapping.
  void clearNodeStat(String id) => _nodeStats.remove(id);

  /// Test helper: force [applyProfileOrder] to report a structured failure.
  bool failApplyProfileOrder = false;

  /// Test helper: publish a result row without running a real job.
  void seedSpeedResult(
    String id,
    int delay,
    double speed, {
    String message = '',
    String ipInfo = '',
  }) {
    _speedResults[id] = speedtest.SpeedTestResultDto(
      indexId: id,
      delay: delay,
      speed: speed,
      message: message,
      ipInfo: ipInfo,
    );
  }

  @override
  Future<void> init() async {}

  @override
  List<ProfileSummary> generate(int count) {
    _summaryCount = count;
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

  void _ensureProfiles() {
    if (_seeded) return;
    _seeded = true;
    // The editor store is seeded with a small deterministic window; the table
    // summary path keeps generating the full `count`.
    final seed = count < 500 ? count : 500;
    for (var i = 0; i < seed; i++) {
      final row = _build(i);
      _profiles.add(
        c.ProfileDto(
          indexId: row.id,
          configType: row.configType,
          coreType: row.coreType,
          configVersion: 4,
          subid: row.subRemarks,
          isSub: true,
          preSocksPort: null,
          displayLog: true,
          remarks: row.remarks,
          address: row.address,
          port: row.port,
          password: '',
          username: '',
          network: row.network,
          muxEnabled: null,
          finalmask: null,
          security: c.SecurityDto(
            streamSecurity: row.streamSecurity == 'none'
                ? null
                : row.streamSecurity,
          ),
          protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
          transportExtra: const c.TransportExtraDto(extraJson: '{}'),
          extraJson: '{}',
        ),
      );
    }
  }

  @override
  List<ProfileSummary> fetchSummaries(int count) {
    final snapshot = fetchProfileSnapshot(count);
    return applyLiveOverlay(snapshot.summaries);
  }

  @override
  ProfileSummaryPage querySummaryPage({
    int cursor = 0,
    int pageSize = kProfileQueryPageSize,
    String? text,
    String? subid,
  }) {
    // Synthetic equivalent of the Rust cursor: slice the generated base rows
    // and expose a real (not fabricated) next cursor at each page boundary.
    final rows = fetchSummaries(_summaryCount);
    final start = cursor < 0 ? 0 : cursor;
    if (start >= rows.length) {
      return ProfileSummaryPage(
        items: const <ProfileSummary>[],
        total: rows.length,
        nextCursor: null,
      );
    }
    final end = (start + pageSize).clamp(start, rows.length);
    return ProfileSummaryPage(
      items: rows.sublist(start, end),
      total: rows.length,
      nextCursor: end < rows.length ? end : null,
    );
  }

  @override
  List<ProfileSummary> queryAllSummaries({String? text, String? subid}) {
    final out = <ProfileSummary>[];
    var cursor = 0;
    for (;;) {
      final page = querySummaryPage(cursor: cursor, text: text, subid: subid);
      out.addAll(page.items);
      final next = page.nextCursor;
      if (next == null || next <= cursor || page.items.isEmpty) break;
      cursor = next;
    }
    return out;
  }

  @override
  ProfileSnapshot fetchProfileSnapshot(
    int count, {
    String? text,
    String? subid,
  }) {
    _ensureProfiles();
    final summaries = orderByPersistedSort(generate(count), _persistedOrder);
    return ProfileSnapshot(
      summaries: summaries,
      profiles: List<c.ProfileDto>.of(_profiles),
    );
  }

  @override
  List<ProfileSummary> applyLiveOverlay(List<ProfileSummary> base) {
    var rows = base;
    if (statsEnabled) {
      rows = applyNodeStatsOverlay(rows, _nodeStats.values);
    }
    return applySpeedTestOverlay(rows, speedTestResults());
  }

  @override
  List<c.ProfileDto> queryAllProfiles() {
    _ensureProfiles();
    return List<c.ProfileDto>.of(_profiles);
  }

  @override
  c.ProfileDto? getProfile(String id) {
    _ensureProfiles();
    for (final p in _profiles) {
      if (p.indexId == id) return p;
    }
    return null;
  }

  @override
  int profileRevision() => _revision;

  @override
  c.SaveProfileResult saveProfile(c.ProfileDto draft, int expectedRevision) {
    _ensureProfiles();
    if (expectedRevision != _revision) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_REVISION_STALE',
          messageKey: 'error.revision_stale',
          retryable: false,
        ),
      );
    }
    if (draft.remarks.trim().isEmpty) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_REQUIRED',
          messageKey: 'error.remarks_required',
          fieldPath: 'remarks',
          retryable: false,
        ),
      );
    }
    var saved = draft;
    if (draft.indexId.trim().isEmpty) {
      saved = _withId(draft, 'syn-new-${_newId++}');
    }
    final index = _profiles.indexWhere((p) => p.indexId == saved.indexId);
    if (index >= 0) {
      _profiles[index] = saved;
    } else {
      _profiles.add(saved);
    }
    _revision += 1;
    return c.SaveProfileResult(
      ok: true,
      profile: saved,
      newRevision: BigInt.from(_revision),
    );
  }

  @override
  c.SaveProfileResult saveImportedProfile(
    c.ProfileDto draft,
    int expectedRevision,
  ) {
    _ensureProfiles();
    if (expectedRevision != _revision) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_REVISION_STALE',
          messageKey: 'error.revision_stale',
          retryable: false,
        ),
      );
    }
    var saved = draft;
    if (draft.indexId.trim().isEmpty) {
      saved = _withId(draft, 'syn-new-${_newId++}');
    }
    final index = _profiles.indexWhere((p) => p.indexId == saved.indexId);
    if (index >= 0) {
      _profiles[index] = saved;
    } else {
      _profiles.add(saved);
    }
    _revision += 1;
    return c.SaveProfileResult(
      ok: true,
      profile: saved,
      newRevision: BigInt.from(_revision),
    );
  }

  /// Test helper: force [deleteProfiles] to report a structured failure so a
  /// caller's error branch (do not clear result rows) can be asserted.
  bool failDeleteProfiles = false;

  @override
  c.DeleteProfilesResult deleteProfiles(List<String> ids) {
    _ensureProfiles();
    if (failDeleteProfiles) {
      return c.DeleteProfilesResult(
        ok: false,
        removed: BigInt.zero,
        error: const c.ErrorDto(
          code: 'E_DELETE_FAILED',
          messageKey: 'error.delete_failed',
          retryable: true,
        ),
      );
    }
    final before = _profiles.length;
    _profiles.removeWhere((p) => ids.contains(p.indexId));
    final removed = before - _profiles.length;
    if (removed > 0) _revision += 1;
    return c.DeleteProfilesResult(ok: true, removed: BigInt.from(removed));
  }

  @override
  c.CopyProfilesResult copyProfiles(List<String> ids) {
    _ensureProfiles();
    final copies = <c.ProfileDto>[];
    for (final id in ids) {
      final source = getProfile(id);
      if (source == null) continue;
      final copy = _withId(
        source,
        'syn-copy-${_newId++}',
        remarks: '${source.remarks} (副本)',
      );
      _profiles.add(copy);
      copies.add(copy);
    }
    if (copies.isNotEmpty) _revision += 1;
    return c.CopyProfilesResult(ok: true, copies: copies);
  }

  @override
  c.SaveProfileResult setProfileRemarks(String id, String remarks) {
    _ensureProfiles();
    if (remarks.trim().isEmpty) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_REQUIRED',
          messageKey: 'error.remarks_required',
          fieldPath: 'remarks',
          retryable: false,
        ),
      );
    }
    final index = _profiles.indexWhere((p) => p.indexId == id);
    if (index < 0) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.not_found',
          retryable: false,
        ),
      );
    }
    final updated = _withId(_profiles[index], id, remarks: remarks);
    _profiles[index] = updated;
    _revision += 1;
    return c.SaveProfileResult(ok: true, profile: updated);
  }

  @override
  c.SimpleResult setActiveProfile(String? id) {
    _active = id;
    return const c.SimpleResult(ok: true);
  }

  @override
  String? getActiveProfile() => _active;

  @override
  c.SimpleResult initEngine(String? dataDir) => const c.SimpleResult(ok: true);

  // -- FIX-03B custom file import (synthetic) ----------------------------

  /// Source paths passed through [customImportFile], in call order. UI-only
  /// evidence of the browse seam; the production bridge does the real copy.
  final List<String> importedCustomFiles = <String>[];

  /// Test helper: force [customImportFile] to report a structured failure.
  bool failCustomImport = false;

  @override
  c.CustomFileResult customImportFile(String sourcePath) {
    importedCustomFiles.add(sourcePath);
    if (failCustomImport || sourcePath.trim().isEmpty) {
      return const c.CustomFileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.custom_file_not_found',
          retryable: false,
        ),
      );
    }
    final dot = sourcePath.lastIndexOf('.');
    final ext = dot > -1 && dot < sourcePath.length - 1
        ? sourcePath.substring(dot)
        : '';
    return c.CustomFileResult(
      ok: true,
      fileName: 'syn-custom-${importedCustomFiles.length}$ext',
    );
  }

  @override
  String dataDir() => 'C:/synthetic-data';

  // -- T09 subscription + import/export (synthetic) ----------------------

  final List<c.SubItemDto> _subs = <c.SubItemDto>[];
  int _subSeq = 0;

  @override
  c.SubsPageDto listSubItems() {
    final items = List<c.SubItemDto>.of(_subs)
      ..sort((a, b) => a.sort.compareTo(b.sort));
    return c.SubsPageDto(items: items);
  }

  @override
  c.SubItemDto? getSubItem(String id) {
    for (final s in _subs) {
      if (s.id == id) return s;
    }
    return null;
  }

  @override
  c.SubItemDtoResult saveSubItem(c.SubItemDto item) {
    final invalid = _validateSub(item);
    if (invalid != null) {
      return c.SubItemDtoResult(ok: false, error: invalid);
    }
    var saved = item;
    if (item.id.trim().isEmpty) {
      saved = _withSubId(item, 'syn-sub-${_subSeq++}');
    }
    final index = _subs.indexWhere((s) => s.id == saved.id);
    if (index >= 0) {
      _subs[index] = saved;
    } else {
      _subs.add(saved);
    }
    return c.SubItemDtoResult(ok: true, item: saved);
  }

  @override
  c.DeleteSubsResult deleteSubItems(List<String> ids) {
    final before = _subs.length;
    _subs.removeWhere((s) => ids.contains(s.id));
    return c.DeleteSubsResult(
      ok: true,
      removed: BigInt.from(before - _subs.length),
    );
  }

  @override
  c.SubItemDtoResult setSubEnabled(String id, bool enabled) {
    final index = _subs.indexWhere((s) => s.id == id);
    if (index < 0) {
      return const c.SubItemDtoResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.not_found',
          retryable: false,
        ),
      );
    }
    final updated = _withSubId(_subs[index], id, enabled: enabled);
    _subs[index] = updated;
    return c.SubItemDtoResult(ok: true, item: updated);
  }

  @override
  c.SimpleResult reorderSubItems(List<String> ids) {
    final byId = <String, c.SubItemDto>{for (final s in _subs) s.id: s};
    final ordered = <c.SubItemDto>[];
    for (var i = 0; i < ids.length; i++) {
      final item = byId.remove(ids[i]);
      if (item != null) ordered.add(_withSubId(item, item.id, sort: i + 1));
    }
    ordered.addAll(byId.values);
    _subs
      ..clear()
      ..addAll(ordered);
    return const c.SimpleResult(ok: true);
  }

  @override
  c.SimpleResult validateSubItem(c.SubItemDto item) {
    final invalid = _validateSub(item);
    return invalid == null
        ? const c.SimpleResult(ok: true)
        : c.SimpleResult(ok: false, error: invalid);
  }

  @override
  void setLocalProxyPort(int? port) {}

  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) async {
    final targets = subIds.isEmpty
        ? _subs.where((s) => s.enabled).toList()
        : _subs.where((s) => subIds.contains(s.id) && s.enabled).toList();
    final entries = <c.SubUpdateEntryDto>[];
    for (final item in targets) {
      if (item.url.trim().isEmpty) {
        entries.add(
          c.SubUpdateEntryDto(
            subId: item.id,
            remarks: item.remarks,
            status: 'skipped',
            message: 'error.url_required',
          ),
        );
        continue;
      }
      // Synthetic success (UI-only): 3 deterministic nodes per
      // subscription. The `synthetic` marker distinguishes this from a
      // real Rust pipeline result; see the class docs.
      entries.add(
        c.SubUpdateEntryDto(
          subId: item.id,
          remarks: item.remarks,
          status: 'updated',
          added: 3,
          existing: 0,
          message: 'synthetic',
        ),
      );
    }
    final success = entries.where((e) => e.status == 'updated').length;
    return c.SubUpdateResult(
      ok: success > 0,
      success: success,
      cancelled: false,
      entries: entries,
      jobId: 'syn-sub-job-${_subSeq++}',
    );
  }

  @override
  Future<c.SubUpdateResult> updateSubscription(String subId, bool viaProxy) =>
      updateSubscriptions(<String>[subId], viaProxy);

  @override
  c.SimpleResult startSubScheduler() => const c.SimpleResult(ok: true);

  @override
  c.SimpleResult stopSubScheduler() => const c.SimpleResult(ok: true);

  @override
  bool subSchedulerRunning() => false;

  @override
  c.JobDto? jobView(String jobId) => null;

  /// Jobs cancelled through the synthetic bridge, in call order. Widget
  /// tests assert against this; it is not a success signal.
  final List<String> cancelledJobs = <String>[];

  @override
  c.CancelResult cancelJob(String jobId) {
    cancelledJobs.add(jobId);
    return const c.CancelResult(outcome: CancelOutcome.requested);
  }

  @override
  Future<c.ImportResult> importFromText(
    String text, {
    String? subid,
    bool deduplicate = true,
  }) async {
    final lines = text
        .split(RegExp(r'\r?\n'))
        .map((l) => l.trim())
        .where((l) => l.isNotEmpty && l.contains('://'))
        .toList();
    final profiles = <c.ProfileDto>[];
    for (var i = 0; i < lines.length; i++) {
      final uri = lines[i];
      if (uri.startsWith('vmess://') ||
          uri.startsWith('vless://') ||
          uri.startsWith('ss://') ||
          uri.startsWith('trojan://') ||
          uri.startsWith('hysteria2://')) {
        profiles.add(
          c.ProfileDto(
            indexId: 'syn-import-${_subSeq++}',
            configType: ConfigType.vless,
            coreType: CoreType.xray,
            configVersion: 4,
            subid: subid ?? '',
            isSub: true,
            displayLog: true,
            remarks: uri.split('#').length > 1
                ? Uri.decodeComponent(uri.split('#').last)
                : 'import-${i + 1}',
            address: '192.0.2.${i + 1}',
            port: 443,
            password: '',
            username: '',
            network: 'raw',
            security: const c.SecurityDto(),
            protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
            transportExtra: const c.TransportExtraDto(extraJson: '{}'),
            extraJson: '{}',
          ),
        );
      }
    }
    return c.ImportResult(
      ok: profiles.isNotEmpty,
      imported: profiles.length,
      profiles: profiles,
      errors: const <c.ParseIssueDto>[],
      error: profiles.isEmpty
          ? const c.ErrorDto(
              code: 'E_FIELD_FORMAT',
              messageKey: 'error.import_nothing',
              retryable: false,
            )
          : null,
    );
  }

  @override
  c.UriParseResult parseShareUri(String line) {
    if (!line.contains('://')) {
      return const c.UriParseResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_FORMAT',
          messageKey: 'error.invalid_uri',
          retryable: false,
        ),
      );
    }
    return c.UriParseResult(
      ok: true,
      profile: c.ProfileDto(
        indexId: 'syn-uri-${_subSeq++}',
        configType: ConfigType.vless,
        coreType: CoreType.xray,
        configVersion: 4,
        subid: '',
        isSub: false,
        displayLog: true,
        remarks: 'uri',
        address: '192.0.2.1',
        port: 443,
        password: '',
        username: '',
        network: 'raw',
        security: const c.SecurityDto(),
        protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
        transportExtra: const c.TransportExtraDto(extraJson: '{}'),
        extraJson: '{}',
      ),
    );
  }

  @override
  Future<c.ShareExportResult> exportProfiles(
    List<String> ids,
    String kind,
  ) async {
    _ensureProfiles();
    final selected = _profiles.where((p) => ids.contains(p.indexId)).toList();
    if (selected.isEmpty) {
      return const c.ShareExportResult(
        ok: false,
        text: '',
        count: 0,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.no_profiles_selected',
          retryable: false,
        ),
      );
    }
    final lines = selected
        .map(
          (p) => 'vless://syn-${p.indexId}@${p.address}:${p.port}#${p.remarks}',
        )
        .toList();
    final joined = lines.join('\n');
    final text = kind == 'base64' ? base64Encode(utf8.encode(joined)) : joined;
    return c.ShareExportResult(ok: true, text: text, count: selected.length);
  }

  /// Test helper: let [writeExportFile] report success so the file-save happy
  /// path can be asserted without touching disk. Defaults to the honest
  /// structured "not wired" failure.
  bool fakeWriteExportFileSucceeds = false;

  /// Paths passed through [writeExportFile], in call order.
  final List<String> writtenExportFiles = <String>[];

  @override
  c.SimpleResult writeExportFile(String path, String text) {
    writtenExportFiles.add(path);
    if (!fakeWriteExportFileSucceeds) {
      // Synthetic default: nothing is written to disk. A structured "not wired"
      // failure (never `ok: true`) so the test double cannot be mistaken for a
      // real export; the production FRB bridge performs the write.
      return const c.SimpleResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_NOT_WIRED',
          messageKey: 'error.not_wired',
          retryable: false,
        ),
      );
    }
    return const c.SimpleResult(ok: true);
  }

  /// Test helper: force [exportClientConfigText] to report a codegen failure.
  bool failExportClientConfig = false;

  /// Ids passed through [exportClientConfigText], in call order.
  final List<String> exportedClientConfigIds = <String>[];

  @override
  c.ShareExportResult exportClientConfigText(String id) {
    _ensureProfiles();
    exportedClientConfigIds.add(id);
    if (failExportClientConfig) {
      return const c.ShareExportResult(
        ok: false,
        text: '',
        count: 0,
        error: c.ErrorDto(
          code: 'E_CODEGEN_FAILED',
          messageKey: 'error.codegen_failed',
          retryable: false,
        ),
      );
    }
    final profile = getProfile(id);
    if (profile == null) {
      return const c.ShareExportResult(
        ok: false,
        text: '',
        count: 0,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.not_found',
          retryable: false,
        ),
      );
    }
    final text =
        '{\n'
        '  "remarks": "${profile.remarks}",\n'
        '  "outbounds": [{"protocol": "vless", "address": "${profile.address}", "port": ${profile.port}}]\n'
        '}';
    return c.ShareExportResult(ok: true, text: text, count: 1);
  }

  // -- T10 group / template (synthetic) ----------------------------------

  final List<groups.FullConfigTemplateDto> _templates =
      <groups.FullConfigTemplateDto>[
        const groups.FullConfigTemplateDto(
          id: 'builtin-xray',
          remarks: 'V2ray',
          enabled: false,
          coreType: CoreType.xray,
        ),
        const groups.FullConfigTemplateDto(
          id: 'builtin-singbox',
          remarks: 'sing-box',
          enabled: false,
          coreType: CoreType.singBox,
        ),
      ];

  @override
  groups.TemplatesPageDto listTemplates() =>
      groups.TemplatesPageDto(items: List.of(_templates));

  @override
  groups.FullConfigTemplateDto? getTemplate(CoreType core) {
    for (final t in _templates) {
      if (t.coreType == core) return t;
    }
    return null;
  }

  @override
  groups.TemplateDtoResult saveTemplate(groups.FullConfigTemplateDto item) {
    final invalid = _validateTemplate(item);
    if (invalid != null) {
      return groups.TemplateDtoResult(ok: false, error: invalid);
    }
    var saved = item;
    if (item.id.trim().isEmpty) {
      saved = _withTemplateId(item, 'syn-tpl-${_subSeq++}');
    }
    final index = _templates.indexWhere((t) => t.id == saved.id);
    if (index >= 0) {
      _templates[index] = saved;
    } else {
      final coreIndex = _templates.indexWhere(
        (t) => t.coreType == saved.coreType,
      );
      if (coreIndex >= 0) {
        saved = _withTemplateId(saved, _templates[coreIndex].id);
        _templates[coreIndex] = saved;
      } else {
        _templates.add(saved);
      }
    }
    return groups.TemplateDtoResult(ok: true, item: saved);
  }

  @override
  c.SimpleResult deleteTemplate(String id) {
    final before = _templates.length;
    _templates.removeWhere((t) => t.id == id);
    final removed = _templates.length != before;
    return removed
        ? const c.SimpleResult(ok: true)
        : const c.SimpleResult(
            ok: false,
            error: c.ErrorDto(
              code: 'E_NOT_FOUND',
              messageKey: 'error.not_found',
              retryable: false,
            ),
          );
  }

  @override
  c.ProfilePageDto groupChildren(String indexId) {
    _ensureProfiles();
    final group = getProfile(indexId);
    if (group == null) {
      return c.ProfilePageDto(items: const [], total: BigInt.zero);
    }
    final byId = <String, c.ProfileDto>{
      for (final p in _profiles) p.indexId: p,
    };
    final ordered = <c.ProfileDto>[];
    final subItems = (group.protoExtra.subChildItems ?? '')
        .split(',')
        .map((s) => s.trim())
        .where((s) => s.isNotEmpty)
        .toList();
    if (subItems.isNotEmpty) {
      final filter = (group.protoExtra.filter ?? '').trim();
      final needles = RegExp(
        filter.replaceAll(RegExp(r'[^A-Za-z\u4e00-\u9fa5]'), '|'),
      );
      for (final p in _profiles) {
        if (!subItems.contains(p.subid)) continue;
        if (filter.isNotEmpty && !needles.hasMatch(p.remarks)) continue;
        ordered.add(p);
      }
    }
    for (final id in (group.protoExtra.childItems ?? '').split(',')) {
      final child = byId[id.trim()];
      if (child != null && !ordered.any((p) => p.indexId == child.indexId)) {
        ordered.add(child);
      }
    }
    return c.ProfilePageDto(items: ordered, total: BigInt.from(ordered.length));
  }

  @override
  c.SaveProfileResult genGroupAll(String subId) {
    _ensureProfiles();
    final sub = getSubItem(subId);
    if (sub == null) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.not_found',
          retryable: false,
        ),
      );
    }
    final draft = c.ProfileDto(
      indexId: '',
      configType: ConfigType.policyGroup,
      coreType: CoreType.xray,
      configVersion: 4,
      subid: subId,
      isSub: false,
      displayLog: true,
      remarks: '${sub.remarks} - PolicyGroup',
      address: '',
      port: 0,
      password: '',
      username: '',
      network: 'raw',
      security: const c.SecurityDto(),
      protoExtra: c.ProtocolExtraDto(
        groupType: 'PolicyGroup',
        subChildItems: subId,
        multipleLoad: 0,
        extraJson: '{}',
      ),
      transportExtra: const c.TransportExtraDto(extraJson: '{}'),
      extraJson: '{}',
    );
    return saveProfile(draft, _revision);
  }

  @override
  groups.GroupGenResult genGroupRegion(String subId) {
    _ensureProfiles();
    const regions = <String>['HK', 'US', 'JP', 'SG', 'TW', 'KR', 'DE'];
    const excluded = <String>['剩余', '过期', '到期', '重置'];
    final created = <c.ProfileDto>[];
    for (final region in regions) {
      final matched = _profiles.where(
        (p) =>
            p.subid == subId &&
            p.remarks.toUpperCase().contains(region) &&
            !excluded.any((w) => p.remarks.contains(w)),
      );
      if (matched.isEmpty) continue;
      final result = saveProfile(
        c.ProfileDto(
          indexId: '',
          configType: ConfigType.policyGroup,
          coreType: CoreType.xray,
          configVersion: 4,
          subid: subId,
          isSub: false,
          displayLog: true,
          remarks: '$subId - $region',
          address: '',
          port: 0,
          password: '',
          username: '',
          network: 'raw',
          security: const c.SecurityDto(),
          protoExtra: c.ProtocolExtraDto(
            groupType: 'PolicyGroup',
            subChildItems: subId,
            filter: region,
            multipleLoad: 0,
            extraJson: '{}',
          ),
          transportExtra: const c.TransportExtraDto(extraJson: '{}'),
          extraJson: '{}',
        ),
        _revision,
      );
      if (result.ok && result.profile != null) created.add(result.profile!);
    }
    return groups.GroupGenResult(ok: true, profiles: created);
  }

  groups.FullConfigTemplateDto _withTemplateId(
    groups.FullConfigTemplateDto t,
    String id,
  ) => groups.FullConfigTemplateDto(
    id: id,
    remarks: t.remarks,
    enabled: t.enabled,
    coreType: t.coreType,
    config: t.config,
    tunConfig: t.tunConfig,
    addProxyOnly: t.addProxyOnly,
    proxyDetour: t.proxyDetour,
  );

  c.ErrorDto? _validateTemplate(groups.FullConfigTemplateDto item) {
    if (item.remarks.trim().isEmpty) {
      return const c.ErrorDto(
        code: 'E_FIELD_REQUIRED',
        messageKey: 'error.remarks_required',
        fieldPath: 'remarks',
        retryable: false,
      );
    }
    if (item.coreType != CoreType.xray && item.coreType != CoreType.singBox) {
      return const c.ErrorDto(
        code: 'E_FIELD_FORMAT',
        messageKey: 'error.template_core_unsupported',
        fieldPath: 'coreType',
        retryable: false,
      );
    }
    for (final entry in {
      'config': item.config,
      'tunConfig': item.tunConfig,
    }.entries) {
      final text = entry.value;
      if (text == null || text.trim().isEmpty) continue;
      if (!_isJsonObject(text) && !_looksYamlMapping(text)) {
        return c.ErrorDto(
          code: 'E_FIELD_FORMAT',
          messageKey: 'error.template_json_invalid',
          fieldPath: entry.key,
          retryable: false,
        );
      }
    }
    return null;
  }

  bool _isJsonObject(String text) {
    try {
      return jsonDecode(text) is Map;
    } catch (_) {
      return false;
    }
  }

  bool _looksYamlMapping(String text) =>
      !text.trimLeft().startsWith(RegExp(r'[[{]')) && text.contains(':');

  c.ErrorDto? _validateSub(c.SubItemDto item) {
    if (item.remarks.trim().isEmpty) {
      return const c.ErrorDto(
        code: 'E_FIELD_REQUIRED',
        messageKey: 'error.remarks_required',
        fieldPath: 'remarks',
        retryable: false,
      );
    }
    if (item.url.trim().isEmpty) {
      return const c.ErrorDto(
        code: 'E_FIELD_REQUIRED',
        messageKey: 'error.url_required',
        fieldPath: 'url',
        retryable: false,
      );
    }
    if (!item.url.startsWith('http://') && !item.url.startsWith('https://')) {
      return const c.ErrorDto(
        code: 'E_FIELD_FORMAT',
        messageKey: 'error.url_invalid',
        fieldPath: 'url',
        retryable: false,
      );
    }
    final headers = item.requestHeaders;
    if (headers != null && headers.trim().isNotEmpty) {
      if (!headers.trimLeft().startsWith('{')) {
        return const c.ErrorDto(
          code: 'E_FIELD_FORMAT',
          messageKey: 'error.sub_headers_invalid',
          fieldPath: 'requestHeaders',
          retryable: false,
        );
      }
    }
    return null;
  }

  c.SubItemDto _withSubId(
    c.SubItemDto s,
    String id, {
    bool? enabled,
    int? sort,
  }) => c.SubItemDto(
    id: id,
    remarks: s.remarks,
    url: s.url,
    moreUrl: s.moreUrl,
    enabled: enabled ?? s.enabled,
    userAgent: s.userAgent,
    requestHeaders: s.requestHeaders,
    sort: sort ?? s.sort,
    filter: s.filter,
    autoUpdateInterval: s.autoUpdateInterval,
    updateTime: s.updateTime,
    convertTarget: s.convertTarget,
    prevProfile: s.prevProfile,
    nextProfile: s.nextProfile,
    preSocksPort: s.preSocksPort,
    memo: s.memo,
    customCoreType: s.customCoreType,
  );

  c.ProfileDto _withId(c.ProfileDto p, String id, {String? remarks}) =>
      c.ProfileDto(
        indexId: id,
        configType: p.configType,
        coreType: p.coreType,
        configVersion: p.configVersion,
        subid: p.subid,
        isSub: p.isSub,
        preSocksPort: p.preSocksPort,
        displayLog: p.displayLog,
        remarks: remarks ?? p.remarks,
        address: p.address,
        port: p.port,
        password: p.password,
        username: p.username,
        network: p.network,
        muxEnabled: p.muxEnabled,
        finalmask: p.finalmask,
        security: p.security,
        protoExtra: p.protoExtra,
        transportExtra: p.transportExtra,
        extraJson: p.extraJson,
      );

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

  // -- T12a settings (synthetic) -----------------------------------------

  Map<String, dynamic> _settings = defaultSettingsJson();
  int _settingsRevision = 0;
  final Map<String, int> _groupRevisions = <String, int>{};

  @override
  settings.SettingsLoadDto getSettings() => settings.SettingsLoadDto(
    ok: true,
    revision: BigInt.from(_settingsRevision),
    groupRevisionsJson: jsonEncode(_groupRevisions),
    settings: null,
    settingsJson: jsonEncode(_settings),
    error: null,
  );

  @override
  settings.SaveSettingsResult saveSettingsJson(
    String settingsJson,
    int expectedRevision,
  ) {
    if (expectedRevision != _settingsRevision) {
      return _staleSettings(expectedRevision);
    }
    final Object? decoded;
    try {
      decoded = jsonDecode(settingsJson);
    } catch (_) {
      return _formatSettings();
    }
    if (decoded is! Map<String, dynamic>) return _formatSettings();
    _settings = decoded;
    _settingsRevision += 1;
    for (final group in defaultSettingsGroups) {
      _groupRevisions[group] = (_groupRevisions[group] ?? 0) + 1;
    }
    return settings.SaveSettingsResult(
      ok: true,
      newRevision: BigInt.from(_settingsRevision),
      changes: const [],
      restartCoreFields: const [],
      restartAppFields: const [],
      nextLaunchFields: const [],
    );
  }

  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String patchJson,
    int expectedRevision,
  ) {
    final current = _groupRevisions[group] ?? 0;
    if (expectedRevision != current) {
      return _staleSettings(expectedRevision);
    }
    final Object? decoded;
    try {
      decoded = jsonDecode(patchJson);
    } catch (_) {
      return _formatSettings();
    }
    _settings[group] = decoded;
    _settingsRevision += 1;
    _groupRevisions[group] = current + 1;
    return settings.SaveSettingsResult(
      ok: true,
      newRevision: BigInt.from(_settingsRevision),
      changes: const [],
      restartCoreFields: const [],
      restartAppFields: const [],
      nextLaunchFields: const [],
    );
  }

  @override
  int settingsRevision() => _settingsRevision;

  settings.SaveSettingsResult _staleSettings(int expected) =>
      settings.SaveSettingsResult(
        ok: false,
        changes: const [],
        restartCoreFields: const [],
        restartAppFields: const [],
        nextLaunchFields: const [],
        error: c.ErrorDto(
          code: 'E_REVISION_STALE',
          messageKey: 'error.revision_stale',
          detail: 'expected=$expected actual=$_settingsRevision',
          retryable: false,
        ),
      );

  settings.SaveSettingsResult _formatSettings() => settings.SaveSettingsResult(
    ok: false,
    changes: const [],
    restartCoreFields: const [],
    restartAppFields: const [],
    nextLaunchFields: const [],
    error: const c.ErrorDto(
      code: 'E_FIELD_FORMAT',
      messageKey: 'error.settings_json',
      retryable: false,
    ),
  );

  // -- T11 routing / DNS (synthetic) -------------------------------------

  // -- T11 routing / DNS (synthetic) -------------------------------------

  final List<routing.RoutingProfileDto> _routings = <routing.RoutingProfileDto>[
    const routing.RoutingProfileDto(
      id: 'syn-rt-white',
      remarks: 'V4-绕过大陆(Whitelist)',
      url: '',
      ruleSet: '[]',
      ruleNum: 0,
      enabled: true,
      locked: false,
      customIcon: '',
      customRulesetPath4Singbox: '',
      domainStrategy: '',
      domainStrategy4Singbox: '',
      sort: 1,
      isActive: true,
    ),
  ];
  final Map<String, List<routing.RoutingRuleDto>> _rules =
      <String, List<routing.RoutingRuleDto>>{};
  final List<dns.DnsProfileDto> _dns = <dns.DnsProfileDto>[
    const dns.DnsProfileDto(
      id: 'syn-dns-xray',
      remarks: 'V2ray',
      enabled: false,
      coreType: CoreType.xray,
      useSystemHosts: false,
    ),
    const dns.DnsProfileDto(
      id: 'syn-dns-sbox',
      remarks: 'sing-box',
      enabled: false,
      coreType: CoreType.singBox,
      useSystemHosts: false,
    ),
  ];
  dns.SimpleDnsDto _simpleDns = const dns.SimpleDnsDto();
  String _ruleMode = 'Rule';
  int _rtSeq = 0;

  @override
  routing.RoutingsPageDto listRoutings() {
    final items = List<routing.RoutingProfileDto>.of(_routings)
      ..sort((a, b) => a.sort.compareTo(b.sort));
    return routing.RoutingsPageDto(items: items);
  }

  @override
  routing.RoutingDtoResult getRouting(String id) {
    for (final r in _routings) {
      if (r.id == id) return routing.RoutingDtoResult(ok: true, item: r);
    }
    return const routing.RoutingDtoResult(
      ok: false,
      error: c.ErrorDto(
        code: 'E_NOT_FOUND',
        messageKey: 'error.not_found',
        retryable: false,
      ),
    );
  }

  @override
  routing.RoutingDtoResult saveRouting(routing.RoutingProfileDto draft) {
    if (draft.remarks.trim().isEmpty) {
      return const routing.RoutingDtoResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_REQUIRED',
          messageKey: 'error.remarks_required',
          fieldPath: 'remarks',
          retryable: false,
        ),
      );
    }
    var saved = draft;
    if (draft.id.trim().isEmpty) {
      saved = _withRoutingId(draft, 'syn-rt-${_rtSeq++}');
    }
    final index = _routings.indexWhere((r) => r.id == saved.id);
    if (index >= 0) {
      _routings[index] = saved;
    } else {
      _routings.add(saved);
    }
    // Mirror the engine: the stored rule list follows the saved RuleSet text,
    // so the single-save path (scheme + rules in one call) reads back.
    _rules[saved.id] = _parseRuleSet(saved.ruleSet);
    return routing.RoutingDtoResult(ok: true, item: saved);
  }

  /// Best-effort parse of a stored `RuleSet` array (domain serde shape).
  /// Unparseable text yields an empty draft list, never a throw.
  List<routing.RoutingRuleDto> _parseRuleSet(String ruleSet) {
    try {
      final decoded = jsonDecode(ruleSet);
      if (decoded is! List) return const <routing.RoutingRuleDto>[];
      final rules = <routing.RoutingRuleDto>[];
      for (final entry in decoded) {
        if (entry is! Map) continue;
        final map = Map<String, Object?>.from(entry);
        List<String> strings(Object? value) {
          if (value is List) {
            return value.map((e) => e.toString()).toList();
          }
          return const <String>[];
        }

        String? text(Object? value) {
          if (value == null) return null;
          final str = value.toString();
          return str.isEmpty ? null : str;
        }

        final id = map['id']?.toString() ?? '';
        if (id.isEmpty) continue;
        final inbound = strings(map['inbound_tag']);
        final ip = strings(map['ip']);
        final domain = strings(map['domain']);
        final protocol = strings(map['protocol']);
        final process = strings(map['process']);
        final ruleType = map['rule_type'];
        rules.add(
          routing.RoutingRuleDto(
            id: id,
            ruleKind: text(map['rule_kind']),
            port: text(map['port']),
            network: text(map['network']),
            inboundTag: inbound,
            hasInboundTag: inbound.isNotEmpty,
            outboundTag: text(map['outbound_tag']),
            ip: ip,
            hasIp: ip.isNotEmpty,
            domain: domain,
            hasDomain: domain.isNotEmpty,
            protocol: protocol,
            hasProtocol: protocol.isNotEmpty,
            process: process,
            hasProcess: process.isNotEmpty,
            enabled: map['enabled'] is bool ? map['enabled'] as bool : true,
            remarks: text(map['remarks']),
            ruleType: ruleType is num ? ruleType.toInt() : null,
          ),
        );
      }
      return rules;
    } catch (_) {
      return const <routing.RoutingRuleDto>[];
    }
  }

  @override
  c.SimpleResult deleteRouting(String id) {
    final before = _routings.length;
    _routings.removeWhere((r) => r.id == id);
    return before == _routings.length
        ? const c.SimpleResult(
            ok: false,
            error: c.ErrorDto(
              code: 'E_NOT_FOUND',
              messageKey: 'error.not_found',
              retryable: false,
            ),
          )
        : const c.SimpleResult(ok: true);
  }

  @override
  c.SimpleResult setDefaultRouting(String id) {
    var found = false;
    for (var i = 0; i < _routings.length; i++) {
      final active = _routings[i].id == id;
      if (active) found = true;
      _routings[i] = _withRoutingActive(_routings[i], active);
    }
    return found
        ? const c.SimpleResult(ok: true)
        : const c.SimpleResult(
            ok: false,
            error: c.ErrorDto(
              code: 'E_NOT_FOUND',
              messageKey: 'error.not_found',
              retryable: false,
            ),
          );
  }

  @override
  routing.RoutingRulesPageDto listRoutingRules(String routingId) =>
      routing.RoutingRulesPageDto(
        ok: true,
        rules: List<routing.RoutingRuleDto>.of(
          _rules[routingId] ?? const <routing.RoutingRuleDto>[],
        ),
        warnings: const [],
      );

  @override
  routing.RoutingDtoResult saveRoutingRules(
    String routingId,
    List<routing.RoutingRuleDto> rules,
  ) {
    _rules[routingId] = List.of(rules);
    return getRouting(routingId);
  }

  @override
  routing.RoutingDtoResult moveRoutingRule(
    String routingId,
    int index,
    int direction,
  ) {
    final list = List<routing.RoutingRuleDto>.of(
      _rules[routingId] ?? const <routing.RoutingRuleDto>[],
    );
    if (index < 0 || index >= list.length) {
      return getRouting(routingId);
    }
    var target = index;
    switch (direction) {
      case 0:
        target = 0;
      case 1:
        target = index - 1;
      case 2:
        target = index + 1;
      case 3:
        target = list.length - 1;
    }
    if (target >= 0 && target < list.length && target != index) {
      final item = list.removeAt(index);
      list.insert(target, item);
      _rules[routingId] = list;
    }
    return getRouting(routingId);
  }

  @override
  routing.RoutingRulesTextResult importRoutingRules(
    String routingId,
    String text,
    bool replace,
  ) {
    try {
      final decoded = jsonDecode(text);
      if (decoded is! List) throw const FormatException();
      final current = replace
          ? <routing.RoutingRuleDto>[]
          : List<routing.RoutingRuleDto>.of(
              _rules[routingId] ?? const <routing.RoutingRuleDto>[],
            );
      for (final entry in decoded) {
        if (entry is! Map) continue;
        current.add(
          routing.RoutingRuleDto(
            id: 'syn-rule-${_rtSeq++}',
            port: entry['port']?.toString(),
            network: entry['network']?.toString(),
            inboundTag: const [],
            hasInboundTag: false,
            outboundTag: entry['outboundTag']?.toString() ?? 'proxy',
            ip: const [],
            hasIp: false,
            domain: const [],
            hasDomain: false,
            protocol: const [],
            hasProtocol: false,
            process: const [],
            hasProcess: false,
            enabled: true,
            remarks: entry['remarks']?.toString(),
          ),
        );
      }
      _rules[routingId] = current;
      return routing.RoutingRulesTextResult(
        ok: true,
        text: '',
        ruleCount: current.length,
      );
    } catch (_) {
      return const routing.RoutingRulesTextResult(
        ok: false,
        text: '',
        ruleCount: 0,
        error: c.ErrorDto(
          code: 'E_FIELD_FORMAT',
          messageKey: 'error.routing_rules_invalid',
          retryable: false,
        ),
      );
    }
  }

  @override
  routing.RoutingRulesTextResult exportRoutingRules(
    String routingId,
    List<String> ids,
  ) {
    final list = _rules[routingId] ?? const [];
    final selected = ids.isEmpty
        ? list
        : list.where((r) => ids.contains(r.id)).toList();
    return routing.RoutingRulesTextResult(
      ok: true,
      text: jsonEncode(
        selected.map((r) => {'outboundTag': r.outboundTag}).toList(),
      ),
      ruleCount: selected.length,
    );
  }

  @override
  routing.RuleModeResult getRuleMode() =>
      routing.RuleModeResult(mode: _ruleMode);

  @override
  c.SimpleResult setRuleMode(String mode) {
    if (mode != 'Rule' && mode != 'Global' && mode != 'Direct') {
      return const c.SimpleResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_RANGE',
          messageKey: 'error.rule_mode_invalid',
          fieldPath: 'mode',
          retryable: false,
        ),
      );
    }
    _ruleMode = mode;
    return const c.SimpleResult(ok: true);
  }

  @override
  dns.DnsPageDto listDns() => dns.DnsPageDto(items: List.of(_dns));

  @override
  dns.DnsDtoResult saveDns(dns.DnsProfileDto draft) {
    if (draft.remarks.trim().isEmpty) {
      return const dns.DnsDtoResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_REQUIRED',
          messageKey: 'error.remarks_required',
          fieldPath: 'remarks',
          retryable: false,
        ),
      );
    }
    var saved = draft;
    if (draft.id.trim().isEmpty) {
      saved = dns.DnsProfileDto(
        id: 'syn-dns-${_rtSeq++}',
        remarks: draft.remarks,
        enabled: draft.enabled,
        coreType: draft.coreType,
        useSystemHosts: draft.useSystemHosts,
        normalDns: draft.normalDns,
        tunDns: draft.tunDns,
        domainStrategy4Freedom: draft.domainStrategy4Freedom,
        domainDnsAddress: draft.domainDnsAddress,
      );
    }
    final index = _dns.indexWhere((d) => d.id == saved.id);
    if (index >= 0) {
      _dns[index] = saved;
    } else {
      _dns.add(saved);
    }
    return dns.DnsDtoResult(ok: true, item: saved);
  }

  @override
  dns.DnsDtoResult importDefaultDns(CoreType core) {
    final index = _dns.indexWhere((d) => d.coreType == core);
    const text = '{"servers": []}';
    if (index >= 0) {
      final current = _dns[index];
      final updated = dns.DnsProfileDto(
        id: current.id,
        remarks: current.remarks,
        enabled: current.enabled,
        coreType: current.coreType,
        useSystemHosts: current.useSystemHosts,
        normalDns: text,
        tunDns: text,
        domainStrategy4Freedom: current.domainStrategy4Freedom,
        domainDnsAddress: current.domainDnsAddress,
      );
      _dns[index] = updated;
      return dns.DnsDtoResult(ok: true, item: updated);
    }
    return const dns.DnsDtoResult(ok: false);
  }

  @override
  dns.SimpleDnsDtoResult loadSimpleDns() => dns.SimpleDnsDtoResult(
    ok: true,
    item: _simpleDns,
    revision: BigInt.from(_settingsRevision),
  );

  @override
  dns.SimpleDnsDtoResult saveSimpleDns(dns.SimpleDnsDto draft, int revision) {
    // Mirror the engine fix: a null globalFakeIp means "not edited here"
    // (this window has no such control), so the stored value survives.
    final merged = dns.SimpleDnsDto(
      useSystemHosts: draft.useSystemHosts,
      addCommonHosts: draft.addCommonHosts,
      fakeIp: draft.fakeIp,
      globalFakeIp: draft.globalFakeIp ?? _simpleDns.globalFakeIp,
      fakeIpRange: draft.fakeIpRange,
      blockBindingQuery: draft.blockBindingQuery,
      blockAaaaQuery: draft.blockAaaaQuery,
      directDns: draft.directDns,
      remoteDns: draft.remoteDns,
      bootstrapDns: draft.bootstrapDns,
      strategy4Freedom: draft.strategy4Freedom,
      strategy4Proxy: draft.strategy4Proxy,
      strategy4ProxyDial: draft.strategy4ProxyDial,
      serveStale: draft.serveStale,
      parallelQuery: draft.parallelQuery,
      hosts: draft.hosts,
      directExpectedIps: draft.directExpectedIps,
      enableHappyEyeballs: draft.enableHappyEyeballs,
    );
    _simpleDns = merged;
    _settingsRevision += 1;
    return dns.SimpleDnsDtoResult(
      ok: true,
      item: _simpleDns,
      revision: BigInt.from(_settingsRevision),
    );
  }

  @override
  dns.RegionalPresetResult applyRegionalPreset(String preset) =>
      dns.RegionalPresetResult(ok: true, preset: preset, pendingUrls: const []);

  @override
  String defaultDnsText(String kind) => '{"servers": []}';

  /// Test helper: model a build/platform without UDP support.
  bool udpSupported = true;

  @override
  speedtest.SpeedTestSupportDto speedTestSupport() =>
      speedtest.SpeedTestSupportDto(
        tcpPing: true,
        realPing: true,
        download: true,
        mixed: true,
        fastRealPing: true,
        udp: udpSupported,
      );

  @override
  c.SimpleResult configureSpeedTest({
    required int pageSize,
    required int mixedConcurrency,
    required int timeoutSecs,
    required String speedTestUrl,
    required String speedPingTestUrl,
    String? ipapiUrl,
    String? udpTestTarget,
    required int delayIntervalSecs,
  }) {
    lastSpeedTestConfig =
        'page=$pageSize mixed=$mixedConcurrency timeout=$timeoutSecs '
        'speed=$speedTestUrl ping=$speedPingTestUrl udp=${udpTestTarget ?? ""}';
    return const c.SimpleResult(ok: true);
  }

  @override
  speedtest.SpeedTestStartDto startSpeedTest(int kind, List<String> indexIds) {
    lastSpeedTestJobId = 'syn-test-${speedTestCalls.length}';
    speedTestCalls.add(<String, Object?>{
      'kind': kind,
      'ids': List<String>.of(indexIds),
    });
    return speedtest.SpeedTestStartDto(
      ok: true,
      jobId: lastSpeedTestJobId,
      total: indexIds.isEmpty ? count : indexIds.length,
    );
  }

  @override
  c.SimpleResult cancelSpeedTest(String jobId) {
    cancelledTestJobs.add(jobId);
    return const c.SimpleResult(ok: true);
  }

  @override
  List<speedtest.SpeedTestResultDto> speedTestResults() =>
      _speedResults.values.toList();

  @override
  int removeInvalidResults() {
    removeInvalidCalls += 1;
    _ensureProfiles();
    // Mirror the Rust orphan-prune: only failed rows whose profile no longer
    // exists are dropped, so another group's (or a failed delete's) evidence
    // is preserved (RE-PROF-06).
    final stored = _profiles.map((p) => p.indexId).toSet();
    final before = _speedResults.length;
    _speedResults.removeWhere((id, r) => r.delay == -1 && !stored.contains(id));
    return before - _speedResults.length;
  }

  /// Group ids passed to [removeInvalidResultsForGroup], in call order.
  final List<String> removeInvalidGroupCalls = <String>[];

  @override
  int removeInvalidResultsForGroup(String subid) {
    removeInvalidGroupCalls.add(subid);
    _ensureProfiles();
    final inGroup = _profiles
        .where((p) => p.subid == subid)
        .map((p) => p.indexId)
        .toSet();
    final before = _speedResults.length;
    _speedResults.removeWhere((id, r) => r.delay == -1 && inGroup.contains(id));
    return before - _speedResults.length;
  }

  /// Tests report zero active jobs so the polling loop settles immediately.
  @override
  int speedTestActiveJobs() => 0;

  /// Orders written through the synthetic bridge, in call order. Widget tests
  /// assert persistence was requested with the real visible id order; it is not
  /// a storage-success signal.
  final List<List<String>> appliedProfileOrders = <List<String>>[];

  @override
  c.SimpleResult applyProfileOrder(List<String> orderedIds) {
    if (orderedIds.isEmpty) return const c.SimpleResult(ok: true);
    appliedProfileOrders.add(List<String>.of(orderedIds));
    if (failApplyProfileOrder) {
      return const c.SimpleResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_ORDER_PERSIST',
          messageKey: 'error.order_persist',
          retryable: true,
        ),
      );
    }
    _persistedOrder
      ..clear()
      ..addAll(orderedIds);
    return const c.SimpleResult(ok: true);
  }

  routing.RoutingProfileDto _withRoutingId(
    routing.RoutingProfileDto r,
    String id,
  ) => routing.RoutingProfileDto(
    id: id,
    remarks: r.remarks,
    url: r.url,
    ruleSet: r.ruleSet,
    ruleNum: r.ruleNum,
    enabled: r.enabled,
    locked: r.locked,
    customIcon: r.customIcon,
    customRulesetPath4Singbox: r.customRulesetPath4Singbox,
    domainStrategy: r.domainStrategy,
    domainStrategy4Singbox: r.domainStrategy4Singbox,
    sort: r.sort,
    isActive: r.isActive,
  );

  routing.RoutingProfileDto _withRoutingActive(
    routing.RoutingProfileDto r,
    bool active,
  ) => routing.RoutingProfileDto(
    id: r.id,
    remarks: r.remarks,
    url: r.url,
    ruleSet: r.ruleSet,
    ruleNum: r.ruleNum,
    enabled: r.enabled,
    locked: r.locked,
    customIcon: r.customIcon,
    customRulesetPath4Singbox: r.customRulesetPath4Singbox,
    domainStrategy: r.domainStrategy,
    domainStrategy4Singbox: r.domainStrategy4Singbox,
    sort: r.sort,
    isActive: active,
  );

  // -- T16 backup / WebDAV / update (synthetic) --------------------------

  /// Every T16 bridge call, in order, for widget-test assertions.
  final List<String> t16Calls = <String>[];
  final List<Map<String, Object?>> webdavUploads = <Map<String, Object?>>[];

  c.WebDavConfigDto webdavConfig = const c.WebDavConfigDto(
    url: '',
    userName: '',
    password: '',
    dirName: 'v2rayN_backup',
  );
  int webdavRevision = 0;
  bool webdavCheckOk = true;
  bool proxyAvailable = false;
  final List<String> installedCores = <String>['xray 26.3.27'];

  c.BackupManifestDto _manifest(String root) => c.BackupManifestDto(
    formatVersion: 1,
    createdAt: 42,
    appSourceCommit: '7d6a967',
    dbSha256: 'deadbeef',
    configSha256: 'feedface',
    root: root,
    resourceCount: 1,
    entityCounts: const <c.EntityCountDto>[],
  );

  @override
  Future<c.BackupResultDto> t16BackupLocal(String destRoot) async {
    t16Calls.add('backup_local:$destRoot');
    return c.BackupResultDto(
      ok: true,
      root: '$destRoot/bundle',
      manifest: _manifest('$destRoot/bundle'),
    );
  }

  @override
  Future<c.BackupListDto> t16BackupList(String parent) async {
    t16Calls.add('backup_list:$parent');
    return c.BackupListDto(items: <c.BackupManifestDto>[_manifest(parent)]);
  }

  @override
  Future<c.VerificationDto> t16BackupVerify(String bundleDir) async {
    t16Calls.add('backup_verify:$bundleDir');
    final broken = bundleDir.contains('broken');
    return c.VerificationDto(
      ok: !broken,
      missing: broken ? const <String>['guiNDB.db'] : const <String>[],
      mismatched: const <String>[],
    );
  }

  @override
  Future<c.RestoreResultDto> t16BackupRestore(String bundleDir) async {
    t16Calls.add('backup_restore:$bundleDir');
    if (bundleDir.contains('broken')) {
      return const c.RestoreResultDto(
        ok: false,
        restored: false,
        message: '',
        error: c.ErrorDto(
          code: 'E_FIELD_FORMAT',
          messageKey: 'error.backup_invalid',
          retryable: false,
        ),
      );
    }
    return const c.RestoreResultDto(
      ok: true,
      restored: true,
      targetBackup: 'guiNDB.db.bak',
      message: 'restore completed',
    );
  }

  @override
  Future<c.RecognitionDto> t16BackupRecognize(String path) async {
    t16Calls.add('backup_recognize:$path');
    return const c.RecognitionDto(
      isUpstream: true,
      hasConfig: true,
      hasDb: true,
      layout: 'guiConfigs/',
      entries: <String>['guiConfigs/guiNConfig.json'],
    );
  }

  @override
  Future<c.ImportSummaryDto> t16BackupImportUpstream(String path) async {
    t16Calls.add('backup_import:$path');
    return c.ImportSummaryDto(
      ok: true,
      status: 'imported',
      sourceVersion: 4,
      importedRows: BigInt.from(12),
      warnings: 0,
      errors: 0,
      message: '导入完成',
    );
  }

  @override
  c.WebDavConfigResultDto t16WebdavConfigGet() => c.WebDavConfigResultDto(
    ok: true,
    config: webdavConfig,
    revision: BigInt.from(webdavRevision),
  );

  @override
  c.WebDavConfigResultDto t16WebdavConfigSave(
    c.WebDavConfigDto cfg,
    int expectedRevision,
  ) {
    t16Calls.add('webdav_config_save');
    if (expectedRevision != webdavRevision) {
      return c.WebDavConfigResultDto(
        ok: false,
        revision: BigInt.from(webdavRevision),
        error: const c.ErrorDto(
          code: 'E_REVISION_STALE',
          messageKey: 'error.revision_stale',
          retryable: false,
        ),
      );
    }
    webdavConfig = cfg;
    webdavRevision += 1;
    return c.WebDavConfigResultDto(
      ok: true,
      config: webdavConfig,
      revision: BigInt.from(webdavRevision),
    );
  }

  @override
  Future<c.WebDavCheckDto> t16WebdavCheck(c.WebDavConfigDto cfg) async {
    t16Calls.add('webdav_check');
    if (!webdavCheckOk) {
      return const c.WebDavCheckDto(
        ok: false,
        createdDir: false,
        status: 401,
        message: '',
        error: c.ErrorDto(
          code: 'E_PERMISSION_DENIED',
          messageKey: 'error.webdav_permission',
          retryable: false,
        ),
      );
    }
    return const c.WebDavCheckDto(
      ok: true,
      createdDir: false,
      status: 207,
      message: 'error.webdav_ok',
    );
  }

  @override
  Future<c.WebDavListDto> t16WebdavList(c.WebDavConfigDto cfg) async {
    t16Calls.add('webdav_list');
    return c.WebDavListDto(
      ok: true,
      items: <c.WebDavEntryDto>[
        c.WebDavEntryDto(
          href: '/v2rayN_backup/backup.zip',
          isDir: false,
          size: BigInt.from(1024),
          modified: '',
        ),
      ],
    );
  }

  @override
  Future<c.WebDavOpDto> t16WebdavBackup(c.WebDavConfigDto cfg) async {
    t16Calls.add('webdav_backup');
    webdavUploads.add(<String, Object?>{'url': cfg.url, 'dir': cfg.dirName});
    return c.WebDavOpDto(
      ok: true,
      bytes: BigInt.from(1024),
      message: 'uploaded',
    );
  }

  @override
  Future<c.RestoreResultDto> t16WebdavRestore(c.WebDavConfigDto cfg) async {
    t16Calls.add('webdav_restore');
    return const c.RestoreResultDto(
      ok: true,
      restored: true,
      targetBackup: 'guiNDB.db.bak',
      message: 'restore completed',
    );
  }

  @override
  List<c.UpdateTargetDto> t16UpdateTargets() => const <c.UpdateTargetDto>[
    c.UpdateTargetDto(
      core: 'v2rayN',
      repo: '2dust/v2rayN',
      supported: true,
      prereleaseCapable: true,
    ),
    c.UpdateTargetDto(
      core: 'xray',
      repo: 'XTLS/Xray-core',
      supported: true,
      prereleaseCapable: true,
    ),
    c.UpdateTargetDto(
      core: 'mihomo',
      repo: 'MetaCubeX/mihomo',
      supported: true,
      prereleaseCapable: false,
    ),
    c.UpdateTargetDto(
      core: 'sing_box',
      repo: 'SagerNet/sing-box',
      supported: true,
      prereleaseCapable: false,
      maxVersion: '1.14.4294967295',
    ),
    c.UpdateTargetDto(
      core: 'tuic',
      repo: '',
      supported: false,
      prereleaseCapable: false,
      note: 'error.update_unsupported',
    ),
  ];

  @override
  Future<c.UpdateReportDto> t16CheckUpdates(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  ) async {
    t16Calls.add('check_updates:${cores.join(",")}:$prerelease:$viaProxy');
    if (viaProxy && !proxyAvailable) {
      return const c.UpdateReportDto(
        ok: false,
        checks: <c.CoreUpdateDto>[],
        error: c.ErrorDto(
          code: 'E_PROXY_UNAVAILABLE',
          messageKey: 'error.proxy_unavailable',
          retryable: false,
        ),
      );
    }
    final checks = <c.CoreUpdateDto>[
      for (final core in cores)
        c.CoreUpdateDto(
          core: core,
          supported: true,
          installedVersion: '26.3.27',
          remoteVersion: '26.4.0',
          hasUpdate: true,
          assetName: '$core-windows-64.zip',
          downloadUrl: 'https://example.invalid/$core.zip',
        ),
    ];
    return c.UpdateReportDto(ok: true, checks: checks);
  }

  @override
  Future<c.ApplyCoreResultDto> t16ApplyCoreUpdate(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  ) async {
    t16Calls.add('apply_core:${cores.join(",")}:$prerelease:$viaProxy');
    if (viaProxy && !proxyAvailable) {
      return const c.ApplyCoreResultDto(
        ok: false,
        applied: <c.AppliedCoreDto>[],
        skipped: <String>[],
        error: c.ErrorDto(
          code: 'E_PROXY_UNAVAILABLE',
          messageKey: 'error.proxy_unavailable',
          retryable: false,
        ),
      );
    }
    return c.ApplyCoreResultDto(
      ok: true,
      applied: <c.AppliedCoreDto>[
        for (final core in cores)
          c.AppliedCoreDto(
            core: core,
            version: '26.4.0',
            installedDir: '/cores/$core',
            keptPrevious: '/cores/$core.previous',
          ),
      ],
      skipped: const <String>[],
    );
  }

  @override
  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec() async {
    t16Calls.add('app_update_spec');
    return const c.ExternalSpecDto(
      ok: true,
      helperExe: '/app/v2rayN-upgrade.exe',
      source: '/app/.staging/v2rayN-7.99.0',
      installRoot: '/app',
      waitForPid: 4242,
      args: <String>['/app/.staging/v2rayN-7.99.0'],
    );
  }

  @override
  c.CleanupResultDto t16CleanupLogsTmp() {
    t16Calls.add('cleanup_logs_tmp');
    return c.CleanupResultDto(
      ok: true,
      deleted: 3,
      bytes: BigInt.from(2048),
      skipped: 1,
    );
  }

  @override
  c.SimpleResult t16OpenConfigDir() {
    t16Calls.add('open_config_dir');
    return const c.SimpleResult(ok: true);
  }

  @override
  c.CoreVersionsDto t16GetCoreVersions() {
    t16Calls.add('get_core_versions');
    return c.CoreVersionsDto(
      items: <c.InstalledCoreDto>[
        for (final entry in installedCores)
          c.InstalledCoreDto(
            core: entry.split(' ').first,
            dir: entry.split(' ').first,
            version: entry.contains(' ') ? entry.split(' ').last : '',
          ),
      ],
    );
  }
}

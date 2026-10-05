import csv
import hashlib
import json
import subprocess
from collections import Counter
from pathlib import Path

import yaml

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
FIELDS = yaml.safe_load((ROOT / 'compat/fields.settings.yaml').read_text(encoding='utf-8'))['items']
COLUMNS = ['id', 'group', 'path', 'ui_control', 'consumer', 'effect', 'effect_time',
           'status', 'issue', 'evidence', 'actual_effect_verified', 'notes', 'upstream', 'row_kind']

preexisting = [
    'crates/application/src/codegen.rs', 'dist/SHA256SUMS', 'dist/build-info.json',
    'services/net_host/src/helper_client.rs', 'services/net_host/src/session.rs',
    'services/privileged_helper/src/main.rs', 'services/privileged_helper/src/windows.rs',
    'docs/evidence/repair/R4-32/host-proxy-incident-2026-10-05.md',
]
baseline = {
    'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
    'upstream': '7d6a967c18c697f28dc6917122ed3a4993fcf336',
    'scope': 'current HEAD plus preexisting working changes; no production source edited',
    'preexisting_working_files': {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in preexisting},
    'execution': 'synthetic-only fault injection and pure configuration generation; no OS proxy/TUN/route/Run-key writes',
}
(OUT / 'baseline.json').write_text(json.dumps(baseline, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')

root_rows = []
for field in FIELDS:
    if not field['source_symbol'].startswith('Config.'):
        continue
    internal = field['id'] in ('FLD-CFG-001', 'FLD-CFG-002')
    root_rows.append({
        'id': field['id'], 'group': 'Config', 'path': field['storage']['key'],
        'ui_control': 'internal state' if internal else 'group container',
        'consumer': 'crates/application/src/engine.rs:1612,1634,1679,1768;crates/bridge_api/src/api/settings.rs:1249,1276,1350',
        'effect': 'upstream keys retained; current selection uses active_index_id/UI store' if internal else 'settings tree persistence; child effects reviewed in leaf rows',
        'effect_time': field['apply_timing'], 'status': 'preserved_only' if internal else 'implemented',
        'issue': 'AUD-ROOT-01;AUD-DESK-02;AUD-ROOT-03',
        'evidence': 'root_contract_repro.log;desktop_repro.log;settings-engine-audit.md',
        'actual_effect_verified': 'no',
        'notes': 'internal state, not an independent user toggle; upstream-key runtime alias not proven' if internal else 'container persistence is not proof that all child options work',
        'upstream': field['source_file'] + ' :: ' + field['source_symbol'],
        'row_kind': 'internal' if internal else 'container',
    })

def write_csv(path, rows):
    with path.open('w', encoding='utf-8-sig', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=COLUMNS, extrasaction='ignore')
        writer.writeheader()
        writer.writerows(rows)

write_csv(OUT / 'container-fields.csv', root_rows)
all_rows = list(root_rows)
for name in ('desktop-fields.csv', 'engine-fields.csv'):
    with (OUT / name).open(encoding='utf-8-sig', newline='') as stream:
        for row in csv.DictReader(stream):
            row['row_kind'] = 'leaf'
            all_rows.append(row)
ids = [r['id'] for r in all_rows]
expected = {f['id'] for f in FIELDS}
assert len(ids) == len(set(ids)) == len(expected) == 180, (len(ids), len(set(ids)), len(expected))
assert set(ids) == expected, (expected - set(ids), set(ids) - expected)
allowed = {'identified', 'implemented', 'verified', 'preserved_only', 'blocked', 'not_applicable'}
assert all(r['status'] in allowed for r in all_rows)
all_rows.sort(key=lambda r: r['id'])
write_csv(OUT / 'settings-effectiveness-180.csv', all_rows)
summary = {
    'ledger_entries': len(all_rows), 'leaf_entries': 155, 'containers': 23, 'internal_state': 2,
    'unique_ids': len(set(ids)), 'missing_ids': [], 'duplicate_ids': [],
    'by_status': dict(Counter(r['status'] for r in all_rows)),
    'leaf_by_status': dict(Counter(r['status'] for r in all_rows if r['row_kind'] == 'leaf')),
    'by_group': dict(Counter(r['group'] for r in all_rows)),
    'note': 'Static review coverage, not a completion percentage or successful real-world effect count.',
}
(OUT / 'matrix-summary.json').write_text(json.dumps(summary, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
print(json.dumps(summary, ensure_ascii=False))

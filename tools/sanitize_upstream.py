#!/usr/bin/env python3
"""Derive sanitized upstream fixtures from a real v2rayN profile directory.

Read-only with respect to the source. Every TEXT value (in the SQLite
databases and in the JSON config) is replaced by a deterministic synthetic
value that preserves:

  * emptiness                 ("" stays "")
  * whether the value is non-ASCII
  * whether the value contains emoji
  * a comparable length magnitude

Table structures (including sqlite-net declared types such as ``varchar`` and
``float``), integer columns (``ConfigVersion`` / ``ConfigType``), row counts and
primary keys are preserved. Embedded-JSON TEXT columns (``ProtoExtra``,
``TransportExtra``, ``Extra``, ``RuleSet``) keep their JSON structure, key names
and numeric/bool/null leaves so the importer can still parse them; only their
string leaves are replaced. The synthetic mapping is content-addressed: the same
source string always maps to the same synthetic string, so cross-column
references (for example ``ProfileItem.Subid`` <-> ``SubItem.Id``) stay
consistent.

The output databases are rebuilt into fresh files (never page-copied), then
``VACUUM``ed, so no original byte can linger in free pages. Finally every
original user-bearing value -- plain TEXT cells plus the string leaves of
embedded-JSON columns -- is scanned for as a substring of the sanitized corpus
and the hit count is printed; a clean run reports 0. JSON keys and numeric/bool
leaves are schema, not payload, and are excluded from the scan.

Do not reverse this script: the mapping is one-way and the fixtures must never
be used to reconstruct real data.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sqlite3
import sys
from pathlib import Path

DEFAULT_SRC = Path(
    r"C:\Users\Colby\AppData\Local\Temp\opencode\v2rayn-upstream-sample"
)
DEFAULT_OUT = Path(__file__).resolve().parents[1] / "fixtures" / "synthetic" / "upstream-real-shape"

# source file -> output file
DB_SOURCES = {
    "guiNDB.db": "guiNDB.db",
    "upstream-old-20260602.db": "upstream-old-v1.db",
    "upstream-old-1780395234.db": "upstream-old-v2.db",
    "upstream-bak-1788539222.db": "upstream-bak.db",
}
CONFIG_SOURCES = {
    "guiNConfig.json": "guiNConfig.json",
}

# Columns whose TEXT values are themselves embedded JSON (upstream serializes
# these CLR blobs with JsonUtils). Their structure -- object/array shape, key
# names, numeric/bool/null leaves -- must survive sanitization, because the
# importer parses them; only leaf *strings* are replaced. Key names are schema,
# not user secrets, so they are preserved verbatim.
JSON_TEXT_COLUMNS = {"ProtoExtra", "TransportExtra", "RuleSet", "Extra"}

ASCII_ALPHABET = "abcdefghijklmnopqrstuvwxyz"

# A pool of common CJK ideographs, deterministic and independent of the source.
_CJK_START, _CJK_END = 0x4E00, 0x9FA5
CJK_POOL = [
    chr(_CJK_START + (i * 733) % (_CJK_END - _CJK_START + 1)) for i in range(1024)
]
EMOJI_POOL = ["\U0001F680", "\U0001F31F", "\U0001F525", "\U0001F308", "\U0001F3AF",
              "\U0001F340", "\U0001F30A", "\U0001F6F0", "\U0001F300", "\U0001F409"]

EMOJI_RE = re.compile(
    "[\U0001F000-\U0001FAFF\U00002600-\U000027BF\U0001F1E6-\U0001F1FF]"
)
NON_ASCII_RE = re.compile("[^\x00-\x7f]")


def _stream(seed: bytes, needed: int) -> bytes:
    out = bytearray()
    counter = 0
    while len(out) < needed:
        out += hashlib.sha256(seed + counter.to_bytes(4, "big")).digest()
        counter += 1
    return bytes(out)


def _generate(value: str, salt: int) -> str:
    """Shape-preserving deterministic replacement for one string."""
    if value == "":
        return ""
    seed = hashlib.sha256(b"v2rayn-fixture-v1\x00" + str(salt).encode() + b"\x00"
                          + value.encode("utf-8")).digest()
    n = len(value)
    has_non_ascii = NON_ASCII_RE.search(value) is not None
    has_emoji = EMOJI_RE.search(value) is not None
    stream = _stream(seed, n * 4 + 16)

    body = []
    for i in range(n):
        if has_non_ascii:
            body.append(CJK_POOL[stream[i] % len(CJK_POOL)])
        else:
            body.append(ASCII_ALPHABET[stream[i] % len(ASCII_ALPHABET)])
    if has_emoji and n > 0:
        pos = stream[n] % n
        body[pos] = EMOJI_POOL[stream[n + 1] % len(EMOJI_POOL)]
    return "".join(body)


class Sanitizer:
    def __init__(self) -> None:
        self._memo: dict[str, str] = {}
        self._used: dict[str, str] = {}
        self.poison: set[str] = set()

    def learn(self, value: str) -> None:
        if value:
            self.poison.add(value)

    def sanitize(self, value: str) -> str:
        if value == "":
            return ""
        cached = self._memo.get(value)
        if cached is not None:
            return cached
        last = None
        for salt in range(0, 50000):
            candidate = _generate(value, salt)
            if candidate in self._used and self._used[candidate] != value:
                continue
            if any(p in candidate for p in self.poison):
                last = candidate
                continue
            self._memo[value] = candidate
            self._used[candidate] = value
            return candidate
        # Extremely unlikely fallback: break any residual match by interleaving.
        candidate = "\u00b7".join(_generate(value, 0))
        self._memo[value] = candidate
        return candidate

    def sanitize_json(self, value: str) -> str:
        """Sanitize the leaf strings of an embedded-JSON TEXT value.

        The JSON structure is preserved so the importer can still parse the
        column; only string leaves are replaced. A value that is not valid JSON
        falls back to plain text sanitization so the fixture never carries an
        original byte sequence.
        """
        stripped = value.strip()
        if stripped[:1] not in ("{", "["):
            return self.sanitize(value)
        try:
            obj = json.loads(value)
        except (ValueError, TypeError):
            return self.sanitize(value)

        def walk(node):
            if isinstance(node, dict):
                return {k: walk(v) for k, v in node.items()}
            if isinstance(node, list):
                return [walk(v) for v in node]
            if isinstance(node, str):
                return self.sanitize(node)
            return node

        return json.dumps(walk(obj), ensure_ascii=False, separators=(",", ":"))


def collect_text_values(conn: sqlite3.Connection) -> list[str]:
    values: list[str] = []
    for (table,) in conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    ).fetchall():
        cols = [
            info[1]
            for info in conn.execute(f'PRAGMA table_info("{table}")')
            if info[2].lower() in ("varchar", "text", "char", "nvarchar", "clob")
        ]
        if not cols:
            continue
        quoted = ",".join('"' + c + '"' for c in cols)
        for row in conn.execute(f'SELECT {quoted} FROM "{table}"'):
            for cell in row:
                if cell is not None and cell != "":
                    values.append(cell)
    return values


def collect_config_strings(obj) -> list[str]:
    values: list[str] = []

    def walk(node) -> None:
        if isinstance(node, dict):
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)
        elif isinstance(node, str) and node:
            values.append(node)

    walk(obj)
    return values


def _json_string_leaves(text: str) -> list[str]:
    """String-leaf values of an embedded-JSON TEXT value.

    Keys are schema (preserved verbatim) and non-string leaves (numbers, bools)
    are structure, so neither is "user text". Only the string leaves can carry
    an imported value and therefore participate in the leak scan.
    """
    try:
        obj = json.loads(text)
    except (ValueError, TypeError):
        return [text] if text else []

    leaves: list[str] = []

    def walk(node) -> None:
        if isinstance(node, dict):
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)
        elif isinstance(node, str) and node:
            leaves.append(node)

    walk(obj)
    return leaves


def collect_payload_values(conn: sqlite3.Connection) -> list[str]:
    """User-bearing values of a database: plain TEXT cells plus the string leaves
    of embedded-JSON TEXT columns. JSON keys and numeric/bool/null leaves are
    deliberately excluded because they are structure, not payload.
    """
    values: list[str] = []
    for (table,) in conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    ).fetchall():
        cols = list(conn.execute(f'PRAGMA table_info("{table}")'))
        text_cols = [
            c[1] for c in cols
            if c[2].lower() in ("varchar", "text", "char", "nvarchar", "clob")
        ]
        if not text_cols:
            continue
        quoted = ",".join('"' + c + '"' for c in text_cols)
        for row in conn.execute(f'SELECT {quoted} FROM "{table}"'):
            for name, cell in zip(text_cols, row):
                if not isinstance(cell, str) or not cell:
                    continue
                if name in JSON_TEXT_COLUMNS:
                    values.extend(_json_string_leaves(cell))
                else:
                    values.append(cell)
    return values


def sanitize_db(src_path: Path, out_path: Path, sanitizer: Sanitizer) -> dict:
    src = sqlite3.connect(f"file:{src_path}?mode=ro", uri=True)
    tables = [
        row[0]
        for row in src.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' "
            "ORDER BY name"
        )
    ]
    schema = [
        row[0]
        for row in src.execute(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' "
            "AND sql IS NOT NULL"
        )
    ]
    if out_path.exists():
        out_path.unlink()
    dst = sqlite3.connect(out_path)
    for stmt in schema:
        dst.execute(stmt)

    stats: dict[str, int] = {}
    for table in tables:
        cols = list(src.execute(f'PRAGMA table_info("{table}")'))
        col_names = [c[1] for c in cols]
        text_cols = {c[1] for c in cols if c[2].lower() in
                     ("varchar", "text", "char", "nvarchar", "clob")}
        quoted = ",".join('"' + c + '"' for c in col_names)
        placeholders = ",".join("?" for _ in col_names)
        insert = f'INSERT INTO "{table}" ({quoted}) VALUES ({placeholders})'
        count = 0
        for row in src.execute(f'SELECT {quoted} FROM "{table}"'):
            new_row = []
            for name, cell in zip(col_names, row):
                if name in JSON_TEXT_COLUMNS and isinstance(cell, str) and cell:
                    new_row.append(sanitizer.sanitize_json(cell))
                elif name in text_cols and isinstance(cell, str):
                    new_row.append(sanitizer.sanitize(cell))
                else:
                    new_row.append(cell)
            dst.execute(insert, new_row)
            count += 1
        stats[table] = count
    dst.commit()
    dst.execute("VACUUM")
    dst.commit()
    dst.close()
    src.close()
    return stats


def scan_hits(blob: str, originals: set[str]) -> list[str]:
    return sorted(v for v in originals if v and v in blob)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--src", type=Path, default=DEFAULT_SRC)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()

    if not args.src.is_dir():
        print(f"source directory not found: {args.src}", file=sys.stderr)
        return 2
    args.out.mkdir(parents=True, exist_ok=True)

    # Pass 1: learn every original text value (poison set for the break loop).
    # The leak scan and the poison set are defined over user-bearing values:
    # plain TEXT cells and the string leaves of embedded-JSON TEXT columns.
    # Embedded-JSON *keys* and numeric/bool/null leaves are schema/structure and
    # are intentionally preserved, so they are neither poisoned nor scanned.
    sanitizer = Sanitizer()
    originals: set[str] = set()
    for name in DB_SOURCES:
        path = args.src / name
        if not path.is_file():
            continue
        conn = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        for value in collect_text_values(conn):
            sanitizer.learn(value)
        for value in collect_payload_values(conn):
            sanitizer.learn(value)
            originals.add(value)
        conn.close()
    for name in CONFIG_SOURCES:
        path = args.src / name
        if not path.is_file():
            continue
        with path.open(encoding="utf-8-sig") as handle:
            obj = json.load(handle)
        for value in collect_config_strings(obj):
            sanitizer.learn(value)
            originals.add(value)

    print(f"learned {len(originals)} distinct original payload values")

    # Pass 2: rebuild the databases.
    for src_name, out_name in DB_SOURCES.items():
        src_path = args.src / src_name
        if not src_path.is_file():
            print(f"  skip missing {src_name}")
            continue
        stats = sanitize_db(src_path, args.out / out_name, sanitizer)
        print(f"  {out_name}: {stats}")

    # Pass 2b: rebuild the config JSON.
    for src_name, out_name in CONFIG_SOURCES.items():
        src_path = args.src / src_name
        if not src_path.is_file():
            continue
        with src_path.open(encoding="utf-8-sig") as handle:
            obj = json.load(handle)

        def walk(node):
            if isinstance(node, dict):
                return {k: walk(v) for k, v in node.items()}
            if isinstance(node, list):
                return [walk(v) for v in node]
            if isinstance(node, str):
                return sanitizer.sanitize(node)
            return node

        with (args.out / out_name).open("w", encoding="utf-8") as handle:
            json.dump(walk(obj), handle, ensure_ascii=False, indent=2)
            handle.write("\n")
        print(f"  {out_name}: sanitized")

    # Pass 3: prove no original user-bearing value survives as a substring of
    # the sanitized corpus. Embedded-JSON keys and non-string leaves are masked
    # out of the comparison because they are preserved schema/structure.
    blob_parts: list[str] = []
    for out_name in list(DB_SOURCES.values()):
        path = args.out / out_name
        if not path.is_file():
            continue
        conn = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
        blob_parts.extend(collect_payload_values(conn))
        conn.close()
    for out_name in CONFIG_SOURCES.values():
        path = args.out / out_name
        if path.is_file():
            with path.open(encoding="utf-8") as handle:
                blob_parts.extend(collect_config_strings(json.load(handle)))
    blob = "\x00".join(blob_parts)

    hits = scan_hits(blob, originals)
    print(f"scan: {len(originals)} original payload values checked, {len(hits)} hits")
    if hits:
        for hit in hits[:20]:
            print(f"  HIT len={len(hit)} sha={hashlib.sha256(hit.encode()).hexdigest()[:12]}")
        return 1
    print("scan: 0 hits -- no original payload value survives in the sanitized corpus")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

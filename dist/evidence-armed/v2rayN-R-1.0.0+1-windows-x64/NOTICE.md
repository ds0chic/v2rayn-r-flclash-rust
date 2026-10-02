# NOTICE

## v2rayN-R is a derived work

`v2rayN-R` is an independent, unofficial reimplementation of
[2dust/v2rayN](https://github.com/2dust/v2rayN) using Flutter (UI) and Rust
(application backend). It is **not** an official v2rayN release and is not
affiliated with or endorsed by the upstream project or its authors.

## Upstream attribution

- Upstream project: **2dust / v2rayN**
- Upstream version this refactor targets: **7.25.4**
- Upstream commit: `7d6a967c18c697f28dc6917122ed3a4993fcf336`
- Upstream license: **GNU General Public License v3.0 (GPL-3.0)**

The frozen upstream source used as the specification baseline is kept outside
the distributed package under `work/` (see `compat/upstream-lock.json` and
`outputs/UPSTREAM_INVENTORY.md`). Portions of behavior, settings schemas, and UI
structure are derived from that source. Per GPL-3.0, the full source of this
derived work is available from this repository; the upstream attribution and
license notices are preserved.

The `LICENSE` file in this repository is the verbatim GPL-3.0 text obtained from
the frozen upstream tree
(`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/LICENSE`,
SHA-256 `C93D1D90B1111EAE8BBC9824405D60186E0CC5F3E643D9FEB3FA66E7629E7F17`).

## What is NOT bundled

This release candidate does **not** bundle any third-party proxy core binary.
In particular:

- **Xray-core** (XTLS/Xray-core) is not included.
- **sing-box** (SagerNet/sing-box) is not included.

Cores are downloaded at runtime by the in-app updater and are pinned by
`tools/cores/cores.lock.json` in the source repository. Each core remains under
its own license and copyright; their releases and licenses are the
responsibility of their respective upstream projects.

The Flutter engine and Dart packages bundled with the app retain their own
licenses; see `data/flutter_assets/NOTICES.Z` inside the package for the
Flutter/Dart third-party notices.

## Trademarks

"v2rayN", "Xray", "sing-box", and all other names are the property of their
respective owners. They are used here only to describe compatibility and origin.
This project uses the distinct name **v2rayN-R** and does not present itself as
an official upstream distribution.

## No warranty

This software is provided "as is", without warranty of any kind, as set out in
the GPL-3.0. It is a work in progress release candidate; see `README.md` for the
current known limitations and platform support.

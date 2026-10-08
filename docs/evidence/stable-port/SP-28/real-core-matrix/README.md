# SP-28 real-core host matrix (live loopback sessions)

Run: 2026-10-07, host Windows 11 x64, repo HEAD `36e7472` + integrator test
hardening. Harness: `crates/application/tests/t10_core_matrix_live.rs`
(`cargo test -p application --locked --test t10_core_matrix_live`), which walks
the real adapter contract (`runtime::adapter::adapter_for` args/env/cwd — the
same functions net_host uses) with synthetic loopback-only configs, probed
ports `>= 11808` (never 10808), and kills every child it spawns.

## Result: 13/14 ok

| core | mode | result | notes |
|---|---|---|---|
| xray | single socks listener | ok | TCP ready, clean stop |
| sing-box | single socks listener | ok | TCP ready, clean stop |
| v2fly | single socks listener | ok | TCP ready, clean stop |
| v2fly_v5 | single socks listener | ok | TCP ready, clean stop |
| mihomo | single socks listener | ok | TCP ready, clean stop |
| hysteria | single socks listener | ok | TCP ready, clean stop |
| naiveproxy | single socks listener | ok | TCP ready, clean stop |
| tuic | single socks listener | ok | TCP ready, clean stop |
| juicity | single socks listener | ok | TCP ready, clean stop |
| brook | single socks listener | ok | TCP ready, clean stop |
| shadowquic | single socks listener | ok | TCP ready, clean stop |
| mieru | single socks listener | ok | TCP ready, clean stop |
| hysteria2 | TLS pair + proxied GET | ok | body `HY2-OK` through the tunnel |
| overtls | TLS pair + proxied GET | **blocked** | see below |

Raw records: `results.json` (PIDs, ports, exit codes, log tails; synthetic
configs and self-signed certs only — no credentials).

## overtls blocked — exact reason (not faked)

The pair starts and both listeners come up, but the proxied GET fails. The
overtls **server** (v0.3.15) refuses the destination:

```
[WARN overtls::server] 127.0.0.1:57707 <> 127.0.0.1:12462 destination address is private, skipping
[ERROR overtls::server] ... failed to create connection to address '127.0.0.1:12462': All addresses failed to connect
```

This is a hardcoded private-destination (SSRF) guard in the locked binary;
`--help` exposes no allow-private switch. The client then drops the tunneled
connection (`http read: ... os error 10054`), so `proxied_body = None`.

Open discrepancy: the PowerShell harness
`tools/cores/session_matrix.ps1` recorded `proxied_http_body = "OV-OK"` for
overtls on 2026-10-06 (`tools/cores/logs/session-overtls.json`) with the same
version/config shape. The difference is not yet explained (candidate causes:
curl's SOCKS request shape vs the Rust SOCKS5 client's IPv4 ATYP, or an
environment difference). Until that is resolved the core stays `blocked`, and
the Rust harness retries the proxied probe up to `PROBE_TIMEOUT` before
recording the block.

## overtls discrepancy — resolved 2026-10-08 (bounded experiment, no re-run)

Root cause: the PS harness never proxied. This host exports
`no_proxy=localhost,127.0.0.1,::1`, and `curl.exe --socks5 127.0.0.1:<CP>
http://127.0.0.1:<TP>/` honors it: verbose output prints
`Uses proxy env variable no_proxy == 'localhost,127.0.0.1,::1'` and connects
DIRECTLY to the loopback HTTP target, bypassing the overtls client entirely.
Reproduced live on a synthetic pair (SP/CP/TP = 12808/12809/12810): PS-style
curl returned `OV-OK` with exit 0 while the overtls server log stayed empty
(0 lines — it never saw the request). The 2026-10-06 `OV-OK` is therefore a
direct-fetch false green, not a proxied session.

The ATYP hypothesis is refuted: with `no_proxy` cleared, curl's real SOCKS5
CONNECT for an IP-literal target is `05 01 00 01 7F 00 00 01 <port>` —
ATYP IPv4 `127.0.0.1`, byte-identical to the Rust `socks5_get`. It would hit
the same private-destination guard. No test-code change warranted;
`socks5_get` stays as is, `LIVE_MATRIX_OK_FLOOR` untouched, `results.json`
not re-run (still 13/14 ok, overtls blocked with reason).

Independent confirmation (integrator, 2026-10-08): with the SOCKS port DEAD (no
overtls client at all), the same curl command still returned `OV-OK` and
verbose printed `* Trying 127.0.0.1:12462...` — a direct connect. The bypass is
reproducible without any proxy present.

The remaining ATYP question is now answered: the Rust live matrix was re-run
once with a DOMAIN-form SOCKS5 destination (ATYP=3, `127.0.0.1`); overtls
stayed blocked with the same server-side private guard (13/14 unchanged, all
other cores green), and the experiment was reverted. Conclusion: the overtls
server rejects loopback destinations regardless of request form, so a
loopback-only harness cannot exercise its proxied path; overtls is finalized
as `blocked` with this root cause. Real overtls acceptance would require a
non-private destination (out of scope for loopback testing).

PS-harness fix applied (2026-10-08): `tools/cores/session_matrix.ps1` line 204
now passes `--noproxy ""` so the curl probe actually traverses the proxy for
loopback targets; the historical `OV-OK` must not be reused as evidence.

## Harness guardrails

- `LIVE_MATRIX_OK_FLOOR = 13`: the test fails if the number of serving cores
  regresses below the measured floor, and every `blocked` entry must carry a
  non-empty reason.
- A blocked core never turns into a silent pass: status/reason/log tail are
  recorded in `results.json`.

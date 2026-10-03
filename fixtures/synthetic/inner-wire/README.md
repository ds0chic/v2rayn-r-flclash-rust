# Inner wire fixtures (FIX-04)

Synthetic upstream-shaped `v2rayn://` payloads (decoded JSON; tests base64url
it into URIs). Shapes follow frozen `InnerFmt.ToUriSingle` / `ResolveSingle`
at `7d6a967`. No real nodes, credentials, or subscription URLs.

- `vless-node.json` — ordinary VLESS with `ProtoExtraObj` /
  `TransportExtraObj` plus one unknown key that must survive the roundtrip.
- `group-node.json` — `PolicyGroup` with `SubChildItems: "self"`,
  `ChildItems` referencing `vless-node.json`'s `IndexId`, numeric
  `MultipleLoad` (3 = RoundRobin).
- `outbound-inline.json` — `Outbound` with inline `CustomOutboundObj` and no
  `Address` (file-type / inline-type must not impersonate each other).
- `invalid-version.json` / `invalid-enum.json` — must be rejected on import.

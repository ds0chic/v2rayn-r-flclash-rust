# T03 case e-core-quick-exit - passed

## Timeline
- 08:25:03.182 net_host pid=42404 ready=True
- 08:25:03.475 apply-bad exit=3 result kind=error code=E_INTERNAL key=error.core_exited
- 08:25:03.491 snapshot state=stopped
- 08:25:03.934 PASS
- 08:25:03.935 cleanup net_host pid=42404

## Detail
```json
{
  "snapshot_state": "stopped",
  "result": {
    "kind": "error",
    "code": "E_INTERNAL",
    "message_key": "error.core_exited",
    "field_path": null,
    "retryable": false,
    "operation_id": "op-1790900703194-1",
    "detail": "core exited early (code Some(23)): [stdout] Xray 26.3.27 (Xray, Penetrates Everything.) d2758a0 (go1.26.1 windows/amd64) | [stdout] A unified platform for anti-censorship. | [stdout] Failed to start: main: failed to load config files: [C:\\Users\\Colby\\AppData\\Local\\Temp\\v2rayn-t03-e-core-quick-exit-20261002-082455\\s-1790900703195-2\\config.json] > infra/conf: failed to build inbound config with tag  > infra/conf: failed to load inbound detour config for protocol not-a-real-protocol > infra/conf: unknown config id: not-a-real-protocol"
  },
  "apply_exit": 3
}
```

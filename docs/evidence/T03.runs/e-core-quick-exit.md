# T03 case e-core-quick-exit - passed

## Timeline
- 01:20:24.476 net_host pid=41868 ready=True
- 01:20:24.837 apply-bad exit=3 result kind=error code=E_INTERNAL key=error.core_exited
- 01:20:24.857 snapshot state=stopped
- 01:20:25.180 PASS
- 01:20:25.181 cleanup net_host pid=41868

## Detail
```json
{
  "snapshot_state": "stopped",
  "apply_exit": 3,
  "result": {
    "kind": "error",
    "code": "E_INTERNAL",
    "message_key": "error.core_exited",
    "field_path": null,
    "retryable": false,
    "operation_id": "op-1790875224492-1",
    "detail": "core exited early (code Some(23)): [stdout] Xray 26.3.27 (Xray, Penetrates Everything.) d2758a0 (go1.26.1 windows/amd64) | [stdout] A unified platform for anti-censorship. | [stdout] Failed to start: main: failed to load config files: [C:\\Users\\Colby\\AppData\\Local\\Temp\\v2rayn-t03-e-core-quick-exit-20261002-012016\\s-1790875224492-2\\config.json] > infra/conf: failed to build inbound config with tag  > infra/conf: failed to load inbound detour config for protocol not-a-real-protocol > infra/conf: unknown config id: not-a-real-protocol"
  }
}
```

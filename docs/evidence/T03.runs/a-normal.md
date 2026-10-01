# T03 case a-normal - passed

## Timeline
- 16:32:58.977 assert port 11808 free
- 16:32:59.857 net_host pid=39116 started ready=True
- 16:33:00.192 t03_client apply exit=0 completed=True
- 16:33:00.871 port 11808 listening owner=24264 xray_pid=24264
- 16:33:00.899 t03_client stop exit=0
- 16:33:01.233 snapshot after stop: snapshot/stopped
- 16:33:01.233 PASS
- 16:33:01.239 cleanup net_host pid=39116

## Detail
```json
{
  "detail": {
    "state": "running",
    "applied_revision": 1,
    "pid": 24264,
    "created_at_ms": 1790843579880,
    "ports": [
      11808
    ],
    "session_id": "s-1790843579876-2",
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "operation_id": "op-1790843579876-1",
    "error": null
  },
  "result": {
    "kind": "accepted",
    "operation_id": "op-1790843579876-1"
  },
  "port_owner_pid": 24264,
  "apply_exit": 0,
  "snapshot_result": {
    "kind": "snapshot",
    "state": "stopped",
    "applied_revision": 1,
    "epoch": 2,
    "last_seq": 7,
    "active_operation": null,
    "recovery": null,
    "host_alive": true
  },
  "xray_created_at_ms": 1790843579880,
  "xray_pid": 24264,
  "stop_exit": 0
}
```

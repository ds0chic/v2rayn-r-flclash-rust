# T03 case a-normal - passed

## Timeline
- 08:24:55.215 assert port 11808 free
- 08:24:55.682 net_host pid=42432 started ready=True
- 08:24:55.997 t03_client apply exit=0 completed=True
- 08:24:56.779 port 11808 listening owner=36500 xray_pid=36500
- 08:24:56.812 t03_client stop exit=0
- 08:24:57.185 snapshot after stop: snapshot/stopped
- 08:24:57.185 PASS
- 08:24:57.185 cleanup net_host pid=42432

## Detail
```json
{
  "xray_pid": 36500,
  "port_owner_pid": 36500,
  "snapshot_result": {
    "kind": "snapshot",
    "state": "stopped",
    "applied_revision": 1,
    "epoch": 2,
    "last_seq": 11,
    "active_operation": null,
    "recovery": null,
    "host_alive": true
  },
  "result": {
    "kind": "accepted",
    "operation_id": "op-1790900695695-1"
  },
  "apply_exit": 0,
  "stop_exit": 0,
  "xray_created_at_ms": 1790900695700,
  "detail": {
    "state": "running",
    "applied_revision": 1,
    "pid": 36500,
    "created_at_ms": 1790900695700,
    "ports": [
      11808
    ],
    "session_id": "s-1790900695695-2",
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "operation_id": "op-1790900695695-1",
    "error": null,
    "tun": null
  }
}
```

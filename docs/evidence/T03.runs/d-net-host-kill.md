# T03 case d-net-host-kill - passed

## Timeline
- 01:20:22.906 net_host pid=37600 ready=True
- 01:20:23.236 apply-hold client pid=39316
- 01:20:23.239 running xray pid=7600 session=s-1790875222927-2 journal_stage=applied
- 01:20:23.240 TerminateProcess net_host pid=37600
- 01:20:23.394 xray pid=7600 died with net_host (KILL_ON_JOB_CLOSE)
- 01:20:23.521 net_host restarted pid=34760 ready=True
- 01:20:24.333 journal after recovery stage=finalized updated=1790875223440
- 01:20:24.333 PASS
- 01:20:24.334 cleanup hold + net_host instances

## Detail
```json
{
  "journal_before": {
    "session_id": "s-1790875222927-2",
    "plan_id": "t03-good-core",
    "desired_revision": 1,
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "stage": "applied",
    "pid": 7600,
    "created_at_ms": 1790875222931,
    "port": 11808,
    "updated_at_ms": 1790875223204
  },
  "session_id": "s-1790875222927-2",
  "detail": {
    "state": "running",
    "applied_revision": 1,
    "pid": 7600,
    "created_at_ms": 1790875222931,
    "ports": [
      11808
    ],
    "session_id": "s-1790875222927-2",
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "operation_id": "op-1790875222927-1",
    "error": null
  },
  "xray_pid": 7600,
  "journal_after": {
    "session_id": "s-1790875222927-2",
    "plan_id": "t03-good-core",
    "desired_revision": 1,
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "stage": "finalized",
    "pid": 7600,
    "created_at_ms": 1790875222931,
    "port": 11808,
    "updated_at_ms": 1790875223440
  }
}
```

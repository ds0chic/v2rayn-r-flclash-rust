# T03 case d-net-host-kill - passed

## Timeline
- 16:33:05.786 net_host pid=40740 ready=True
- 16:33:06.115 apply-hold client pid=35600
- 16:33:06.119 running xray pid=29052 session=s-1790843585805-2 journal_stage=applied
- 16:33:06.120 TerminateProcess net_host pid=40740
- 16:33:06.287 xray pid=29052 died with net_host (KILL_ON_JOB_CLOSE)
- 16:33:06.415 net_host restarted pid=29364 ready=True
- 16:33:07.230 journal after recovery stage=finalized updated=1790843586328
- 16:33:07.231 PASS
- 16:33:07.232 cleanup hold + net_host instances

## Detail
```json
{
  "detail": {
    "state": "running",
    "applied_revision": 1,
    "pid": 29052,
    "created_at_ms": 1790843585809,
    "ports": [
      11808
    ],
    "session_id": "s-1790843585805-2",
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "operation_id": "op-1790843585804-1",
    "error": null
  },
  "session_id": "s-1790843585805-2",
  "xray_pid": 29052,
  "journal_after": {
    "session_id": "s-1790843585805-2",
    "plan_id": "t03-good-core",
    "desired_revision": 1,
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "stage": "finalized",
    "pid": 29052,
    "created_at_ms": 1790843585809,
    "port": 11808,
    "updated_at_ms": 1790843586328
  },
  "journal_before": {
    "session_id": "s-1790843585805-2",
    "plan_id": "t03-good-core",
    "desired_revision": 1,
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "stage": "applied",
    "pid": 29052,
    "created_at_ms": 1790843585809,
    "port": 11808,
    "updated_at_ms": 1790843586083
  }
}
```

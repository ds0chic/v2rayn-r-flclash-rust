# T03 case d-net-host-kill - passed

## Timeline
- 08:25:01.613 net_host pid=12012 ready=True
- 08:25:01.942 apply-hold client pid=40684
- 08:25:01.946 running xray pid=35076 session=s-1790900701631-2 journal_stage=applied
- 08:25:01.947 TerminateProcess net_host pid=12012
- 08:25:02.113 xray pid=35076 died with net_host (KILL_ON_JOB_CLOSE)
- 08:25:02.239 net_host restarted pid=37740 ready=True
- 08:25:03.043 journal after recovery stage=finalized updated=1790900702154
- 08:25:03.044 PASS
- 08:25:03.045 cleanup hold + net_host instances

## Detail
```json
{
  "journal_before": {
    "session_id": "s-1790900701631-2",
    "plan_id": "t03-good-core",
    "desired_revision": 1,
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "stage": "applied",
    "pid": 35076,
    "created_at_ms": 1790900701635,
    "port": 11808,
    "updated_at_ms": 1790900701910
  },
  "journal_after": {
    "session_id": "s-1790900701631-2",
    "plan_id": "t03-good-core",
    "desired_revision": 1,
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "stage": "finalized",
    "pid": 35076,
    "created_at_ms": 1790900701635,
    "port": 11808,
    "updated_at_ms": 1790900702154
  },
  "session_id": "s-1790900701631-2",
  "xray_pid": 35076,
  "detail": {
    "state": "running",
    "applied_revision": 1,
    "pid": 35076,
    "created_at_ms": 1790900701635,
    "ports": [
      11808
    ],
    "session_id": "s-1790900701631-2",
    "config_sha256": "9da10a2ece14b0a5b55363557d05e97fb1c755fce9e29d86f10e847640c776b3",
    "operation_id": "op-1790900701631-1",
    "error": null,
    "tun": null
  }
}
```

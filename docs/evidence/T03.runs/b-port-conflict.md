# T03 case b-port-conflict - passed

## Timeline
- 16:33:01.250 occupied 127.0.0.1:11808 with a local TcpListener
- 16:33:01.378 net_host pid=41964 ready=True
- 16:33:01.401 apply exit=3 result kind=error code=E_PORT_CONFLICT
- 16:33:01.420 snapshot state=stopped
- 16:33:01.420 PASS
- 16:33:01.421 cleanup listener + net_host

## Detail
```json
{
  "snapshot_state": "stopped",
  "apply_exit": 3,
  "result": {
    "kind": "error",
    "code": "E_PORT_CONFLICT",
    "message_key": "error.port_conflict",
    "field_path": "port",
    "retryable": false,
    "operation_id": "op-1790843581392-1",
    "detail": "127.0.0.1:11808 unavailable: 通常每个套接字地址(协议/网络地址/端口)只允许使用一次。 (os error 10048)"
  }
}
```

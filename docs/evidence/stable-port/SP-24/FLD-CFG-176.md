# SP-24.FLD-CFG-176 — SimpleDNSItem.EnableHappyEyeballs（门控修好）

唯一流程：DNS 窗关开关→保存→restart_core→出站 `sockopt` 无 `happyEyeballs` 块。
原版：`V2rayDnsService.cs:532-554`（`FillSockoptDomainStrategy`，`7d6a967`）。
修法：`config_codegen/src/xray/dns.rs` 门控改 `params != default` 为
`enable_happy_eyeballs == true`；freedom/dial 双调用点透传
`simple.enable_happy_eyeballs`（`dns_to_codegen` 早已透传该旗，`codegen.rs:578`）。
合同：`sp24_happy_off_hides_block_freedom` /
`sp24_happy_off_hides_block_dial`（修前红：仍输出块；修后绿），
`sp24_happy_on_emits_params` 回归绿。
diff：OFF+`300/true/2/3` 修前含块，修后仅 `domainStrategy`。
正式 UI/FRB/重启/xray 二进制/重开：未验证。

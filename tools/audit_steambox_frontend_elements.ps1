param(
    [string]$SourceRoot = 'C:\Users\Colby\.gemini\antigravity\scratch\SteamTools',
    [string]$Destination = 'docs\decisions\steambox-frontend-elements-2026-10-02.csv'
)
$ErrorActionPreference = 'Stop'
$viewRoots = @(
    'src/BD.WTTS.Client.Avalonia/UI/Views',
    'src/BD.WTTS.Client.Plugins.Accelerator/UI/Views',
    'src/BD.WTTS.Client.Plugins.GameAccount/UI/Views',
    'src/BD.WTTS.Client.Plugins.GameList/UI/Views',
    'src/BD.WTTS.Client.Plugins.SteamIdleCard/UI/Views'
)
$rows = [System.Collections.Generic.List[object]]::new()
$fieldPolicies = @{}
$actionPolicies = @()
$actionPolicyPath = Join-Path $PWD 'docs/decisions/STEAMBOX_FRONTEND_ACTION_POLICIES.md'
if (Test-Path -LiteralPath $actionPolicyPath) {
    $actionPolicies = @(Get-Content -LiteralPath $actionPolicyPath | Where-Object { $_ -match '^\| SB-A-[0-9]{3} \|' } | ForEach-Object {
        $cells = $_.Split('|')
        [pscustomobject]@{ id = $cells[1].Trim(); sourceClause = $cells[2].Trim() }
    })
}
$fieldPolicyPath = Join-Path $PWD 'docs/decisions/STEAMBOX_FRONTEND_FIELD_POLICIES.md'
if (Test-Path -LiteralPath $fieldPolicyPath) {
    $fieldPolicyBody = Get-Content -LiteralPath $fieldPolicyPath -Raw
    $fieldClassByPrefix = @{ G = 'GeneralSettings'; U = 'UISettings'; S = 'SteamSettings'; P = 'ProxySettings'; A = 'GameAccountSettings'; L = 'GameLibrarySettings'; I = 'SteamIdleSettings'; X = 'GameAcceleratorSettings' }
    foreach ($entry in [regex]::Matches($fieldPolicyBody, '(?m)^\| ([GUSPALIX])-([0-9]+) \| ([A-Za-z][A-Za-z0-9_]*) \|')) {
        $fieldClass = $fieldClassByPrefix[$entry.Groups[1].Value]
        $fieldPolicies[$fieldClass + '.' + $entry.Groups[3].Value] = $entry.Groups[1].Value + '-' + $entry.Groups[2].Value
    }
    if ($fieldPolicies.Count -ne 101) { throw 'Field policy document must contain exactly the 101 frozen seed keys; append new discovery separately.' }
}
$ordinal = 0
foreach ($viewRoot in $viewRoots) {
    $sourceFiles = Get-ChildItem -LiteralPath (Join-Path $SourceRoot $viewRoot) -Filter '*.axaml' -File -Recurse | Sort-Object FullName
    foreach ($sourceFile in $sourceFiles) {
        $relative = [System.IO.Path]::GetRelativePath($SourceRoot, $sourceFile.FullName).Replace('\', '/')
        $scope = if ($relative -match 'Plugins\.Accelerator/') { 'network' } elseif ($relative -match 'Plugins\.GameAccount/') { 'accounts' } elseif ($relative -match 'Plugins\.GameList/') { 'library' } elseif ($relative -match 'Plugins\.SteamIdleCard/') { 'idle' } else { 'shell_settings' }
        $applicability = 'candidate_requires_route_check'
        if ($relative -match 'Debug(Page|Window)\.axaml$|SteamFamilyShareManagePage\.axaml$|Plugins\.Accelerator/UI/Views/Pages/AcceleratorPage\.axaml$|Plugins\.SteamIdleCard/UI/Views/Pages/MainFramePage\.axaml$') { $applicability = 'not_applicable_source_inactive' }
        elseif ($relative -match 'Pages/(MainView|HomePage|Settings/[^/]+|Common/[^/]+|About/[^/]+)\.axaml$|Controls/(TitleBar|SplashScreen)\.axaml$|Windows/(MainWindow|ContentWindow)\.axaml$|Plugins\.(Accelerator|GameAccount|GameList|SteamIdleCard)/') { $applicability = 'in_scope_source_view' }
        $xmlSettings = [System.Xml.XmlReaderSettings]::new()
        $xmlSettings.DtdProcessing = [System.Xml.DtdProcessing]::Prohibit
        $xmlSettings.XmlResolver = $null
        $reader = [System.Xml.XmlReader]::Create($sourceFile.FullName, $xmlSettings)
        try { $document = [System.Xml.Linq.XDocument]::Load($reader, [System.Xml.Linq.LoadOptions]::SetLineInfo) } finally { $reader.Dispose() }
        foreach ($element in $document.Descendants()) {
            $ordinal++
            $kind = $element.Name.LocalName
            $attributes = @{}
            foreach ($attribute in $element.Attributes()) { if (-not $attribute.IsNamespaceDeclaration) { $attributes[$attribute.Name.LocalName] = $attribute.Value } }
            $command = @($element.Attributes() | Where-Object { $_.Name.LocalName -match 'Command$|^CommandParameter$' } | ForEach-Object { $_.Name.LocalName + '=' + $_.Value }) -join ' | '
            $handlers = @($element.Attributes() | Where-Object { $_.Name.LocalName -match 'Click$|Tapped$|Invoked$|Changed$|Requested$|KeyDown$|KeyUp$|Closing$|Closed$|Opened$|Pointer[A-Za-z]+$|Loaded$|GotFocus$|LostFocus$|^Drop$|^Drag[A-Za-z]+$' } | ForEach-Object { $_.Name.LocalName + '=' + $_.Value }) -join ' | '
            $bindings = @($element.Attributes() | Where-Object { $_.Value -match '\{(Binding|CompiledBinding|DynamicResource|x:Static|x:Bind)' } | ForEach-Object { $_.Name.LocalName + '=' + $_.Value }) -join ' | '
            $rules = [System.Collections.Generic.List[string]]::new()
            $role = 'layout_or_declarative'
            if ($kind.Contains('.') -or $kind -match '^(Style|Styles|Setter|ControlTemplate|DataTemplate|ResourceDictionary|SolidColorBrush|LinearGradientBrush|GradientStop)$') { $rules.Add('P00') }
            elseif ($kind -match 'TextBlock|SelectableTextBlock|Label|Badge') { $role = 'display'; $rules.Add('P01') }
            elseif ($kind -match 'Image|Avatar|Icon|Geometry') { $role = 'image'; $rules.Add('P02') }
            elseif ($kind -match 'Button|MenuItem') { $role = 'command'; $rules.Add('P03') }
            elseif ($kind -match 'TextBox|PasswordBox|AutoCompleteBox|NumericUpDown|NumberBox') { $role = 'input'; $rules.Add('P08') }
            elseif ($kind -match 'ToggleSwitch|CheckBox|ToggleButton|RadioButton') { $role = 'value'; $rules.Add('P10') }
            elseif ($kind -match 'ComboBox|Slider|ColorPicker|DatePicker|TimePicker') { $role = 'value'; $rules.Add('P10') }
            elseif ($kind -match 'NavigationView|TabView|TabStrip|TabControl') { $role = 'navigation'; $rules.Add('P11') }
            elseif ($kind -match 'ListBox|ListView|ItemsControl|TreeView|DataGrid|ItemsRepeater|AppItem') { $role = 'collection'; $rules.Add('P13') }
            elseif ($kind -match 'Progress|Chart|Wave') { $role = 'progress'; $rules.Add('P21') }
            elseif ($kind -match 'Window|TitleBar|TrayIcon') { $role = 'window'; $rules.Add('P24') }
            else { $rules.Add('P00') }
            if ($command -match 'Refresh|Reload|CheckUpdate') { $rules.Add('P05') }
            if ($command -match 'StartProxy|RunOrStop|IdleRunStartOrStop') { $rules.Add('P04') }
            if ($handlers -match 'DoubleTapped') { $rules.Add('P14') }
            if ($kind -match 'MenuItem|ContextMenu') { $rules.Add('P16') }
            if ($handlers -match 'Drop|Drag') { $rules.Add('P17') }
            if ($command -match 'Delete|ClearAll|Reset') { $rules.Add('P20') }
            if ($command -match 'Select.*Path|Select.*Image|Open.*Folder|Open.*Url|Open.*Directory') { $rules.Add('P19') }
            if ($scope -eq 'idle' -and $command -match 'Login|CookieLogin') { $rules.Add('P23') }
            if ($command.Length -gt 0 -or $handlers.Length -gt 0) { $rules.Add('P03') }
            if ($role -eq 'input' -and $bindings -match 'SearchText') { $rules.Add('P09') }
            if ($role -in 'input','value' -and $relative -match 'EditAppInfoPage|AchievementWindow') { $rules.Add('P07') }
            if ($role -in 'input','value' -and $relative -match 'IdleSteamLoginPage') { $rules.Add('P23') }
            if ($role -eq 'value' -and $bindings -match 'IsAllCheck|IsCheckAll|ThreeStateEnable') { $rules.Add('P15') }
            if ($relative -match 'SettingsPage.axaml$' -and $role -eq 'navigation') { $rules.Add('P12') }
            $allSettingKeys = @([regex]::Matches($bindings, '(GeneralSettings|UISettings|SteamSettings|ProxySettings|SteamIdleSettings|GameAccountSettings|GameLibrarySettings|GameAcceleratorSettings)\.([A-Za-z][A-Za-z0-9_]*)') | ForEach-Object { $_.Value } | Select-Object -Unique)
            $fieldKeys = @($allSettingKeys | Where-Object { $fieldPolicies.Count -eq 0 -or $fieldPolicies.ContainsKey($_) }) -join ' | '
            $fieldPolicyIds = @($allSettingKeys | Where-Object { $fieldPolicies.ContainsKey($_) } | ForEach-Object { $fieldPolicies[$_] }) -join ' | '
            $computedSettingKeys = @($allSettingKeys | Where-Object { $fieldPolicies.Count -gt 0 -and -not $fieldPolicies.ContainsKey($_) }) -join ' | '
            $fieldPolicy = if ($fieldKeys.Length -gt 0) { 'STEAMBOX_FRONTEND_FIELD_POLICIES.md:' + $fieldPolicyIds + '；别名须绑定同一字段ID' }
                elseif ($role -eq 'input' -and $bindings -match 'SearchText') { 'query_only:IME结束后<=100ms合并；不写设置、不提交业务' }
                elseif ($relative -match 'EditAppInfoPage|AchievementWindow' -and $role -in 'input','value') { 'dialog_commit:独立draft；取消不写；保存成功后才提交效果' }
                elseif ($relative -match 'IdleSteamLoginPage' -and $role -in 'input','value') { 'challenge_draft:仅显式登录提交；成功须验证会话与SID；秘密不写视图恢复状态' }
                elseif ($role -eq 'value' -and $bindings -match 'IsAllCheck|IsCheckAll|ThreeStateEnable') { 'selection_or_domain_set:范围按对应页面动作表；不凭Toggle控件推断运行已生效' }
                elseif ($role -in 'input','value') { 'STEAMBOX_FRONTEND_FLOW_SPEC.md:按具体页面/绑定行；无唯一策略则阻止实现放行' }
                else { 'not_a_field' }
            $journeys = switch ($scope) {
                'network' { 'SB-J02,SB-J03,SB-J04,SB-J05,SB-J17' }
                'accounts' { 'SB-J06,SB-J07,SB-J15' }
                'library' { 'SB-J08,SB-J09,SB-J10,SB-J11,SB-J12,SB-J14,SB-J15,SB-J17' }
                'idle' { 'SB-J13,SB-J14,SB-J15,SB-J17' }
                default { 'SB-J01,SB-J16,SB-J17' }
            }
            $guard = switch ($scope) {
                'network' { 'networkRevision+runtimeEpoch；SDK另带gameId/areaId/operationId' }
                'accounts' { 'platformId+accountId+accountEpoch；切换全局排他' }
                'library' { 'accountEpoch+SteamID+AppId+resourceVersion；queryId拒绝旧查询' }
                'idle' { 'accountEpoch+WebSteamID+nativeSteamID+runEpoch；三者核实后才启动' }
                default { 'window/viewKey+fieldRevision；pure UI不发Rust业务operation' }
            }
            $feedback = switch ($role) {
                'command' { '按下下一帧反馈；接受≠保存≠生效；忙状态仅锁冲突域；错误留在当前上下文' }
                'input' { '输入/IME与选区立即保留；校验就地；保存失败保留草稿；不阻塞键入' }
                'value' { '当前输入与确认值区分；失败按字段策略恢复；禁用原因可见' }
                'collection' { '刷新保留可靠内容、stable-ID选择和scrollAnchor；隐藏目标不得参与未标明范围的批量动作' }
                'navigation' { '同页幂等；切页不重启/停止任务；返回保留筛选、锚点和焦点' }
                'progress' { '真实进度/阶段；未知总量不定进度；隐藏停止绘制而业务继续' }
                'window' { '关闭/隐藏/退出按托盘配置；单实例唤回不重复worker' }
                'image' { '可见区异步加载；固定占位；失败同尺寸fallback；不导致布局跳动' }
                'display' { '只读权威投影；目标/版本一致；重要状态不只用颜色' }
                default { '布局/样式无业务副作用；不遮挡输入、不进入Tab序列' }
            }
            $symbols = [System.Collections.Generic.List[string]]::new()
            foreach ($attribute in $element.Attributes()) {
                if ($attribute.Name.LocalName -match 'Command$') {
                    $bindingMatch = [regex]::Match($attribute.Value, '^\{(?:Binding|CompiledBinding)\s+(?:Path=)?([^,}]+)')
                    if ($bindingMatch.Success) {
                        $symbolMatch = [regex]::Match($bindingMatch.Groups[1].Value.Trim(), '(?:^|\.)([A-Za-z_][A-Za-z0-9_]*)$')
                        if ($symbolMatch.Success) { $symbols.Add($symbolMatch.Groups[1].Value) }
                    }
                }
            }
            foreach ($handlerMatch in [regex]::Matches($handlers, '=[ ]*([A-Za-z_][A-Za-z0-9_]*)')) { $symbols.Add($handlerMatch.Groups[1].Value) }
            $sourceName = [System.IO.Path]::GetFileName($relative)
            $policyIds = [System.Collections.Generic.List[string]]::new()
            $unresolvedSymbols = [System.Collections.Generic.List[string]]::new()
            foreach ($symbol in @($symbols | Select-Object -Unique)) {
                $literal = [char]96 + $symbol + [char]96
                $candidates = @($actionPolicies | Where-Object { $_.sourceClause.Contains($literal) })
                $specific = @($candidates | Where-Object { $_.sourceClause.Contains($sourceName) })
                if ($specific.Count -gt 0) { $candidates = $specific }
                if ($candidates.Count -eq 0) { $unresolvedSymbols.Add($symbol) }
                else { foreach ($candidate in $candidates) { $policyIds.Add($candidate.id) } }
            }
            $actionPolicyIds = @($policyIds | Select-Object -Unique) -join ' | '
            $actionContract = if ($actionPolicyIds.Length -gt 0) { 'STEAMBOX_FRONTEND_ACTION_POLICIES.md:' + $actionPolicyIds + '；按source/容器/参数分发，候选不自动启用' }
                elseif ($symbols.Count -gt 0) { '未解析入口：须补确切action合同，阻止流程放行' }
                else { '无command/handler；按P规则、字段策略和动态入口追加规则处理，不造Rust operation' }
            $line = ([System.Xml.IXmlLineInfo]$element).LineNumber
            $ancestry = [System.Collections.Generic.List[string]]::new()
            $current = $element
            while ($null -ne $current) {
                if ($null -eq $current.Parent) { $position = 1 }
                else {
                    $sameKind = @($current.Parent.Elements() | Where-Object { $_.Name -eq $current.Name })
                    $position = [Array]::IndexOf($sameKind, $current) + 1
                }
                $ancestry.Insert(0, ($current.Name.LocalName + '[' + $position + ']'))
                $current = $current.Parent
            }
            $selector = $ancestry -join '/'
            $hashInput = [System.Text.Encoding]::UTF8.GetBytes($relative + '#' + $selector)
            $hash = [System.Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($hashInput)).Substring(0, 16).ToLowerInvariant()
            $rows.Add([pscustomobject]@{
                element_id = 'SB-E-' + $hash
                scope = $scope
                source = $relative
                line = $line
                selector = $selector
                element_type = $kind
                name = $attributes['Name']
                command = $command
                handlers = $handlers
                bindings = $bindings
                visible_condition = $attributes['IsVisible']
                enabled_condition = $attributes['IsEnabled']
                role = $role
                base_rules = (@($rules | Select-Object -Unique) -join '+')
                field_keys = $fieldKeys
                field_policy_ids = $fieldPolicyIds
                computed_setting_keys = $computedSettingKeys
                field_policy = $fieldPolicy
                context_guard = $guard
                feedback_contract = $feedback
                focus_return = 'P18/P19：还原触发元素；失效则最近可用；刷新/异步结果不得抢焦点'
                journey_candidates = $journeys
                action_symbols = @($symbols | Select-Object -Unique) -join ' | '
                action_policy_ids = $actionPolicyIds
                unresolved_action_symbols = @($unresolvedSymbols | Select-Object -Unique) -join ' | '
                action_contract = $actionContract
                applicability = $applicability
                status = if ($applicability -eq 'not_applicable_source_inactive') { 'not_applicable' } else { 'identified' }
                runtime_verification = '未运行'
            })
        }
    }
}
$destinationPath = [System.IO.Path]::GetFullPath((Join-Path $PWD $Destination))
if (-not $destinationPath.StartsWith([System.IO.Path]::GetFullPath((Join-Path $PWD 'docs')) + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) { throw 'Output must stay under docs.' }
[void][System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($destinationPath))
$rows | Export-Csv -LiteralPath $destinationPath -NoTypeInformation -Encoding utf8
$stats = $rows | Group-Object scope | Select-Object Name,Count
$stats | ConvertTo-Json -Compress
Write-Output ('ElementRows=' + $rows.Count)

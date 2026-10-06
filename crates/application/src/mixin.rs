//! mihomo (`CoreConfigClashService`) custom-config merge (T10).
//!
//! The mihomo core only runs user-supplied YAML (`custom_only_partial` in
//! `compat/features.yaml`); this module ports the pure merge step of
//! `GenerateClientCustomConfig` so it can be tested without a kernel:
//! - `mixed-port` / `log-level` / `external-controller` rewrite, `secret`
//!   removal, `allow-lan` / `bind-address`, `ipv6`, default `mode`;
//! - optional TUN section merge;
//! - user mixin merge with `prepend-` / `append-` / `removed-` list
//!   directives (`ModifyContentMerge`).
//!
//! File IO stays with the caller; tests cover merge output plus a write/read
//! round-trip through a temp file.

use domain::{codes, DomainError};
use serde_yaml::{Mapping, Value};

/// Resolved network facts for one mihomo generation run.
#[derive(Debug, Clone)]
pub struct MixinOptions {
    pub local_port: i32,
    pub state_port2: i32,
    pub log_level: String,
    pub allow_lan: bool,
    pub ipv6: bool,
    pub tun_enabled: bool,
    pub mixin_enabled: bool,
}

impl Default for MixinOptions {
    fn default() -> Self {
        Self {
            // Never the live 10808 proxy port.
            local_port: 11808,
            state_port2: 11810,
            log_level: "warning".into(),
            allow_lan: false,
            ipv6: false,
            tun_enabled: false,
            mixin_enabled: true,
        }
    }
}

fn key(name: &str) -> Value {
    Value::String(name.to_string())
}

fn parse_mapping(text: &str, field: &str) -> Result<Mapping, DomainError> {
    let value: Value = serde_yaml::from_str(text).map_err(|error| {
        DomainError::new(codes::FIELD_FORMAT, "error.mixin_yaml_invalid")
            .with_field(field)
            .with_detail(error.to_string())
    })?;
    match value {
        Value::Mapping(map) => Ok(map),
        _ => Err(
            DomainError::new(codes::FIELD_FORMAT, "error.mixin_yaml_mapping_required")
                .with_field(field),
        ),
    }
}

/// Upstream `GetLogLevel`: `none` becomes mihomo `silent`.
pub fn clash_log_level(level: &str) -> &str {
    if level == "none" {
        "silent"
    } else {
        level
    }
}

/// Merge `base_yaml` with the runtime rewrites, an optional TUN section and
/// an optional user mixin. Returns the merged YAML text.
pub fn generate_mihomo(
    base_yaml: &str,
    mixin_yaml: Option<&str>,
    tun_yaml: Option<&str>,
    opts: &MixinOptions,
) -> Result<String, DomainError> {
    let mut base = parse_mapping(base_yaml, "base")?;

    base.insert(key("mixed-port"), Value::Number(opts.local_port.into()));
    base.insert(
        key("log-level"),
        Value::String(clash_log_level(&opts.log_level).to_string()),
    );
    base.insert(
        key("external-controller"),
        Value::String(format!("127.0.0.1:{}", opts.state_port2)),
    );
    base.remove(key("secret"));
    if opts.allow_lan {
        base.insert(key("allow-lan"), Value::String("true".into()));
        base.insert(key("bind-address"), Value::String("*".into()));
    } else {
        base.insert(key("allow-lan"), Value::String("false".into()));
    }
    base.insert(key("ipv6"), Value::Bool(opts.ipv6));
    base.entry(key("mode"))
        .or_insert(Value::String("Rule".into()));

    if opts.tun_enabled {
        if let Some(tun) = tun_yaml {
            let tun_map = parse_mapping(tun, "tun")?;
            if let Some(section) = tun_map.get(key("tun")) {
                base.insert(key("tun"), section.clone());
            }
        }
    }

    if opts.mixin_enabled {
        if let Some(mixin) = mixin_yaml {
            if !mixin.trim().is_empty() {
                let mixin_map = parse_mapping(mixin, "mixin")?;
                apply_mixin(&mut base, mixin_map, opts.tun_enabled);
            }
        }
    }

    serde_yaml::to_string(&Value::Mapping(base)).map_err(|error| {
        DomainError::new(codes::INTERNAL, "error.mixin_serialize_failed")
            .with_detail(error.to_string())
    })
}

fn apply_mixin(base: &mut Mapping, mixin: Mapping, tun_enabled: bool) {
    for (key, value) in mixin {
        let name = match key.as_str() {
            Some(name) => name.to_string(),
            None => continue,
        };
        if !tun_enabled && name == "tun" {
            continue;
        }
        if name.starts_with("prepend-")
            || name.starts_with("append-")
            || name.starts_with("removed-")
        {
            merge_list_directive(base, &name, value);
        } else {
            base.insert(Value::String(name), value);
        }
    }
}

fn merge_list_directive(base: &mut Mapping, directive: &str, value: Value) {
    let (target, prepend, removed) = if let Some(rest) = directive.strip_prefix("prepend-") {
        (rest, true, false)
    } else if let Some(rest) = directive.strip_prefix("append-") {
        (rest, false, false)
    } else if let Some(rest) = directive.strip_prefix("removed-") {
        (rest, false, true)
    } else {
        return;
    };
    let items = match value {
        Value::Sequence(items) => items,
        _ => return,
    };
    if removed {
        if let Some(Value::Sequence(current)) = base.get_mut(key(target)) {
            current.retain(|existing| {
                let text = yaml_scalar_text(existing);
                !items.iter().any(|drop| {
                    let prefix = yaml_scalar_text(drop);
                    !prefix.is_empty() && text.starts_with(&prefix)
                })
            });
        }
        return;
    }
    match base.get_mut(key(target)) {
        None => {
            base.insert(key(target), Value::Sequence(items));
        }
        Some(Value::Sequence(current)) => {
            if prepend {
                for item in items.into_iter().rev() {
                    current.insert(0, item);
                }
            } else {
                current.extend(items);
            }
        }
        Some(_) => {}
    }
}

fn yaml_scalar_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        _ => serde_yaml::to_string(value)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str =
        "port: 7890\nmode: direct\nsecret: hunter2\nrules:\n  - DOMAIN,example.com,DIRECT\n";

    #[test]
    fn rewrites_runtime_keys() {
        let out = generate_mihomo(BASE, None, None, &MixinOptions::default()).unwrap();
        let map = parse_mapping(&out, "out").unwrap();
        assert_eq!(
            map.get(key("mixed-port")),
            Some(&Value::Number(11808.into()))
        );
        assert_eq!(
            map.get(key("external-controller")),
            Some(&Value::String("127.0.0.1:11810".into()))
        );
        assert!(!map.contains_key(key("secret")));
        assert!(!out.contains("10808"));
    }

    #[test]
    fn none_log_level_becomes_silent() {
        let opts = MixinOptions {
            log_level: "none".into(),
            ..MixinOptions::default()
        };
        let out = generate_mihomo(BASE, None, None, &opts).unwrap();
        assert!(out.contains("silent"));
    }

    #[test]
    fn mixin_overwrites_and_merges_lists() {
        let mixin = "mode: rule\nprepend-rules:\n  - DOMAIN,ads.example,BLOCK\nappend-rules:\n  - MATCH,DIRECT\n";
        let out = generate_mihomo(BASE, Some(mixin), None, &MixinOptions::default()).unwrap();
        let map = parse_mapping(&out, "out").unwrap();
        assert_eq!(map.get(key("mode")), Some(&Value::String("rule".into())));
        let rules = map.get(key("rules")).unwrap().as_sequence().unwrap();
        assert_eq!(rules.len(), 3);
        assert!(yaml_scalar_text(&rules[0]).contains("ads.example"));
    }

    #[test]
    fn removed_directive_drops_matching_prefix() {
        let mixin = "removed-rules:\n  - DOMAIN,example.com\n";
        let out = generate_mihomo(BASE, Some(mixin), None, &MixinOptions::default()).unwrap();
        let map = parse_mapping(&out, "out").unwrap();
        let rules = map.get(key("rules")).unwrap().as_sequence().unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn tun_section_merges_only_when_enabled() {
        let tun = "tun:\n  enable: true\n";
        let off = generate_mihomo(BASE, None, Some(tun), &MixinOptions::default()).unwrap();
        assert!(!off.contains("enable"));
        let opts = MixinOptions {
            tun_enabled: true,
            ..MixinOptions::default()
        };
        let on = generate_mihomo(BASE, None, Some(tun), &opts).unwrap();
        assert!(on.contains("enable"));
    }

    #[test]
    fn ipv6_rewrite_follows_switch_unconditionally() {
        // Upstream `CoreConfigClashService` assigns `fileContent["ipv6"]`
        // unconditionally, so OFF overwrites a carried `ipv6: true`.
        let base = "ipv6: true\nmode: rule\n";
        let on = generate_mihomo(
            base,
            None,
            None,
            &MixinOptions {
                ipv6: true,
                ..MixinOptions::default()
            },
        )
        .unwrap();
        assert!(on.contains("ipv6: true"), "{on}");
        let off = generate_mihomo(base, None, None, &MixinOptions::default()).unwrap();
        assert!(off.contains("ipv6: false"), "{off}");
    }

    #[test]
    fn mixin_merge_skipped_when_disabled() {
        let mixin = "rules:\n  - MATCH,DIRECT\n";
        let opts = MixinOptions {
            mixin_enabled: false,
            ..MixinOptions::default()
        };
        let out = generate_mihomo(BASE, Some(mixin), None, &opts).unwrap();
        assert!(!out.contains("MATCH,DIRECT"), "{out}");
    }

    #[test]
    fn rejects_non_mapping_base() {
        let err = generate_mihomo(
            "- just\n- a\n- list\n",
            None,
            None,
            &MixinOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err.code, codes::FIELD_FORMAT);
    }

    // SP-24 G-06 (generator-side consumer proof): a bad mixin is a hard
    // `FIELD_FORMAT` error so the engine keeps the old plan; unknown keys
    // are retained verbatim (upstream merge order).
    #[test]
    fn sp24_rejects_bad_mixin_and_retains_unknown_keys() {
        let err = generate_mihomo(
            BASE,
            Some("rules:\n\t- tab-indent\n"),
            None,
            &MixinOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err.code, codes::FIELD_FORMAT);

        let out = generate_mihomo(
            BASE,
            Some("unknown-kept: 42\n"),
            None,
            &MixinOptions::default(),
        )
        .unwrap();
        assert!(out.contains("unknown-kept"), "{out}");
    }
}

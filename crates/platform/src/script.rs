//! Custom system-proxy PAC/script path fields.
//!
//! Plan section 15 requires preserving the user-configured custom proxy script
//! as a real feature, but this task only carries and validates the two path
//! fields (`CustomSystemProxyPacPath`, `CustomSystemProxyScriptPath`). Scripts
//! are **never executed here**; execution belongs to the wiring stage under an
//! explicit, user-controlled environment.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{PlatformError, Result};

/// Persisted field name for the custom PAC file.
pub const CUSTOM_PAC_FIELD: &str = "CustomSystemProxyPacPath";
/// Persisted field name for the custom system proxy script.
pub const CUSTOM_SCRIPT_FIELD: &str = "CustomSystemProxyScriptPath";

/// Carrier for the two custom system-proxy paths.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CustomSystemProxySetting {
    #[serde(default)]
    pub pac_path: Option<String>,
    #[serde(default)]
    pub script_path: Option<String>,
}

impl CustomSystemProxySetting {
    pub fn new(pac_path: Option<String>, script_path: Option<String>) -> Self {
        Self {
            pac_path,
            script_path,
        }
    }

    /// Validate both paths when present. Does nothing when a field is unset.
    pub fn validate(&self) -> Result<()> {
        validate_existing_file(self.pac_path.as_deref())?;
        validate_existing_file(self.script_path.as_deref())?;
        Ok(())
    }

    /// Resolve the PAC file, if configured. Validates existence.
    pub fn pac_file(&self) -> Result<Option<PathBuf>> {
        validate_existing_file(self.pac_path.as_deref())
    }

    /// Resolve the custom script, if configured. Validates existence only.
    pub fn script_file(&self) -> Result<Option<PathBuf>> {
        validate_existing_file(self.script_path.as_deref())
    }
}

/// Ensure a configured path points at an existing file.
///
/// * `None` / empty            -> `Ok(None)`
/// * existing regular file     -> `Ok(Some(path))`
/// * missing path              -> `Err(NotFound)`
/// * existing directory/other  -> `Err(Invalid)`
pub fn validate_existing_file(path: Option<&str>) -> Result<Option<PathBuf>> {
    let Some(raw) = path.filter(|p| !p.trim().is_empty()) else {
        return Ok(None);
    };
    let candidate = PathBuf::from(raw);
    if candidate.is_file() {
        Ok(Some(candidate))
    } else if candidate.exists() {
        Err(PlatformError::Invalid(format!(
            "{raw} is not a regular file"
        )))
    } else {
        Err(PlatformError::NotFound(raw.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_paths_are_valid() {
        let setting = CustomSystemProxySetting::default();
        assert!(setting.validate().is_ok());
        assert_eq!(setting.pac_file().expect("ok"), None);
        assert_eq!(setting.script_file().expect("ok"), None);
    }

    #[test]
    fn missing_path_is_not_found() {
        let setting = CustomSystemProxySetting::new(Some("Z:\\nope\\missing.pac".into()), None);
        assert!(matches!(
            setting.validate(),
            Err(PlatformError::NotFound(_))
        ));
    }
}

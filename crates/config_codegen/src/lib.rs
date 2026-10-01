//! Xray / sing-box structured config generators (T07/T08).
//!
//! The crate is a pure transformation: `CodegenInput -> GeneratedConfigs`.
//! It never starts a process, never touches the system state and performs no
//! IO itself (callers provide the sample fixtures as data).

pub mod diff;
pub mod input;
pub mod singbox;
pub mod util;
pub mod xray;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use input::{CodegenInput, ConfigType};

/// Structured generator error. `field_path` points at the input field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodegenError {
    pub code: String,
    pub message: String,
    pub field_path: Option<String>,
}

impl CodegenError {
    pub fn new(code: &str, message: impl Into<String>, field_path: Option<&str>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            field_path: field_path.map(ToString::to_string),
        }
    }

    pub fn unsupported_combination(message: impl Into<String>, field_path: &str) -> Self {
        Self::new("unsupported_combination", message, Some(field_path))
    }

    pub fn missing_required_field(message: impl Into<String>, field_path: &str) -> Self {
        Self::new("missing_required_field", message, Some(field_path))
    }

    pub fn dangling_reference(message: impl Into<String>, field_path: &str) -> Self {
        Self::new("dangling_reference", message, Some(field_path))
    }

    pub fn invalid_reference(message: impl Into<String>, field_path: &str) -> Self {
        Self::new("invalid_reference", message, Some(field_path))
    }

    pub fn custom_outbound_missing(message: impl Into<String>, field_path: &str) -> Self {
        Self::new("custom_outbound_missing", message, Some(field_path))
    }

    /// The generator refuses to emit the host's reserved live proxy port.
    pub fn reserved_port(message: impl Into<String>, field_path: &str) -> Self {
        Self::new("reserved_port", message, Some(field_path))
    }
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.field_path {
            Some(path) => write!(f, "{}: {} ({})", self.code, self.message, path),
            None => write!(f, "{}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for CodegenError {}

/// Severity of a generation diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    Info,
    Warning,
}

/// Non-fatal note produced by the generator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub code: String,
    pub message: String,
    pub field_path: Option<String>,
}

impl Diagnostic {
    pub fn info(code: &str, message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Info,
            code: code.to_string(),
            message: message.into(),
            field_path: None,
        }
    }

    pub fn warning(code: &str, message: impl Into<String>, field_path: Option<&str>) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            code: code.to_string(),
            message: message.into(),
            field_path: field_path.map(ToString::to_string),
        }
    }
}

/// One generated file (name + content).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub name: String,
    pub content: String,
}

/// Result of one generation run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedConfigs {
    /// Parsed main configuration object.
    pub main: Value,
    /// Additional files (empty for both current generators).
    pub files: Vec<GeneratedFile>,
    pub diagnostics: Vec<Diagnostic>,
}

impl GeneratedConfigs {
    pub fn new(main: Value, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            main,
            files: vec![],
            diagnostics,
        }
    }
}

/// Generate the Xray client configuration.
pub fn generate_xray(input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError> {
    xray::generate(input)
}

/// Generate the sing-box client configuration.
pub fn generate_singbox(input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError> {
    singbox::generate(input)
}

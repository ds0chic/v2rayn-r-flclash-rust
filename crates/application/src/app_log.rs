//! App diagnostic file logger (upstream `ServiceLib.Common.Logging`).
//!
//! Consumes the canonical `GuiItem.EnableLog` (`FLD-CFG-063`, default true).
//! This is the *application* log (`guiLogs/<shortdate>.txt`); it must not be
//! confused with `CoreBasicItem.LogEnabled` (`FLD-CFG-026`), which controls
//! core (kernel) logging instead.
//!
//! Upstream semantics mirrored here:
//! - destination `<app>/guiLogs/<shortdate>.txt` (a new file per date is the
//!   upstream rotation; a size cap with numbered backups is added because the
//!   NLog `FileTarget` in the frozen tree sets no `archiveAboveSize`);
//! - `LoggingEnabled(false)` suspends all file writes (`SaveLog` early-out);
//! - the recovery journal (`recoverable_commit`) never consults this gate, so
//!   it stays usable while logging is off.
//!
//! The enabled flag is always derived from the canonical settings at the call
//! site (the engine reads `settings.gui_item.enable_log` live). A failed save
//! rolls the settings back, so the logger can never disagree with storage:
//! there is no separate flag to go stale.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::AppSettings;

/// Upstream `Utils.GetLogPath`: the log directory under the app directory.
pub const GUI_LOG_DIR_NAME: &str = "guiLogs";

/// Size rotation threshold: the current day-file is archived before a write
/// that would push it past this many bytes.
pub const DEFAULT_ROTATE_BYTES: u64 = 512 * 1024;

/// How many archived day-files (`<date>.txt.1`, …) are kept per date.
pub const DEFAULT_KEEP_ROTATED: usize = 5;

/// Replacement text for redacted secrets.
pub const REDACTED: &str = "[redacted]";

/// Node-link schemes: a token with one of these schemes is a node credential
/// or subscription address and is never written to disk.
const NODE_SCHEMES: [&str; 9] = [
    "vmess://",
    "vless://",
    "ss://",
    "trojan://",
    "tuic://",
    "hysteria://",
    "hysteria2://",
    "hy2://",
    "wireguard://",
];

/// `key=value` / `key: value` names whose value is a secret. Compared
/// case-insensitively against the exact key (no prefix matching, so `url`
/// alone stays visible while `sub_url` is redacted).
const SECRET_KEYS: [&str; 13] = [
    "password",
    "passwd",
    "pwd",
    "token",
    "secret",
    "authorization",
    "cookie",
    "uuid",
    "private-key",
    "privatekey",
    "psk",
    "sub_url",
    "suburl",
];

/// Rotation/file policy. The enabled flag is intentionally not stored here;
/// callers pass it per write from the canonical settings.
#[derive(Clone, Debug)]
pub struct AppLogService {
    rotate_bytes: u64,
    keep_rotated: usize,
    dir_override: Option<PathBuf>,
}

impl Default for AppLogService {
    fn default() -> Self {
        Self::new()
    }
}

impl AppLogService {
    pub fn new() -> Self {
        Self {
            rotate_bytes: DEFAULT_ROTATE_BYTES,
            keep_rotated: DEFAULT_KEEP_ROTATED,
            dir_override: None,
        }
    }

    /// Synthetic-only rotation policy (tests).
    pub fn with_limits(rotate_bytes: u64, keep_rotated: usize) -> Self {
        Self {
            rotate_bytes: rotate_bytes.max(1),
            keep_rotated,
            dir_override: None,
        }
    }

    /// Test/embedding hook: write under `dir/guiLogs` instead of the engine
    /// data directory. Production never sets this.
    pub fn set_dir_override(&mut self, dir: impl Into<PathBuf>) {
        self.dir_override = Some(dir.into());
    }

    /// Resolve the log root: the override when set, else
    /// `<data_dir>/guiLogs`. `None` means nowhere to write (in-memory engine
    /// without an override); the caller must skip the write.
    pub fn log_root(&self, data_dir: Option<&Path>) -> Option<PathBuf> {
        if let Some(dir) = &self.dir_override {
            return Some(dir.join(GUI_LOG_DIR_NAME));
        }
        data_dir.map(|dir| dir.join(GUI_LOG_DIR_NAME))
    }

    /// Current day-file for `date` (`YYYY-MM-DD`, upstream `${shortdate}`).
    pub fn current_file(root: &Path, date: &str) -> PathBuf {
        root.join(format!("{date}.txt"))
    }

    /// Archived copy `n` (1-based) of a day-file.
    pub fn rotated_file(path: &Path, n: usize) -> PathBuf {
        let mut name = path.file_name().map(|s| s.to_owned()).unwrap_or_default();
        name.push(format!(".{n}"));
        path.with_file_name(name)
    }

    /// Append one redacted line. `Ok(false)` without touching the filesystem
    /// when `enabled` is false (upstream `SaveLog` early-out). `root` is the
    /// resolved `guiLogs` directory (see [`Self::log_root`]).
    pub fn append(&self, root: &Path, date: &str, enabled: bool, line: &str) -> io::Result<bool> {
        if !enabled {
            return Ok(false);
        }
        fs::create_dir_all(root)?;
        let path = Self::current_file(root, date);
        self.rotate_if_needed(&path)?;
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(file, "{}", redact_line(line))?;
        Ok(true)
    }

    /// Archive the day-file when it reached the size threshold, keeping at
    /// most `keep_rotated` archives (ownership cleanup: older copies are
    /// deleted, everything stays inside the log root).
    fn rotate_if_needed(&self, path: &Path) -> io::Result<()> {
        let len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if len < self.rotate_bytes {
            return Ok(());
        }
        if self.keep_rotated == 0 {
            fs::remove_file(path)?;
            return Ok(());
        }
        for n in (1..self.keep_rotated).rev() {
            let from = Self::rotated_file(path, n);
            if from.exists() {
                let _ = fs::rename(&from, Self::rotated_file(path, n + 1));
            }
        }
        fs::rename(path, Self::rotated_file(path, 1))?;
        Ok(())
    }
}

/// Canonical projection: the file logger is active exactly when the stored
/// `GuiItem.EnableLog` is true.
pub fn enabled_from_settings(settings: &AppSettings) -> bool {
    settings.gui_item.enable_log
}

/// Strip node credentials and subscription addresses from one log line
/// (synthetic secrets in tests; never real user data). Redacted in order:
/// URI userinfo (`scheme://user:pass@host` -> `scheme://[redacted]@host`),
/// whole node-link tokens (`vmess://…` -> `vmess://[redacted]`), then
/// `key=value` / `key: value` pairs for [`SECRET_KEYS`].
pub fn redact_line(line: &str) -> String {
    let no_userinfo = redact_userinfo(line);
    let no_links = redact_node_links(&no_userinfo);
    redact_secret_values(&no_links)
}

fn redact_userinfo(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(scheme_end) = match_scheme(line, i) {
            let rest = &line[scheme_end..];
            let at = rest.find('@');
            let slash = rest.find(['/', ' ', '\t', '"', '\'']);
            match (at, slash) {
                (Some(a), slash) if slash.is_none_or(|s| a < s) && a > 0 => {
                    out.push_str(&line[i..scheme_end]);
                    out.push_str(REDACTED);
                    out.push('@');
                    i = scheme_end + a + 1;
                    continue;
                }
                _ => {}
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Length of `scheme://` at `line[i..]`, or `None`.
fn match_scheme(line: &str, i: usize) -> Option<usize> {
    let rest = &line[i..];
    let sep = rest.find("://")?;
    let scheme = &rest[..sep];
    if !scheme.is_empty()
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
    {
        Some(i + sep + 3)
    } else {
        None
    }
}

fn redact_node_links(line: &str) -> String {
    line.split_inclusive([' ', '\t'])
        .map(|chunk| {
            let (core, trail) = split_trailing_punct(chunk);
            let lower = core.to_ascii_lowercase();
            if NODE_SCHEMES.iter().any(|s| lower.starts_with(s)) {
                let scheme = &core[..core.find("://").map(|p| p + 3).unwrap_or(core.len())];
                format!("{scheme}{REDACTED}{trail}")
            } else {
                chunk.to_string()
            }
        })
        .collect()
}

fn split_trailing_punct(token: &str) -> (&str, &str) {
    let end = token
        .trim_end_matches(|c: char| {
            matches!(
                c,
                '.' | ',' | ';' | '!' | '?' | '"' | '\'' | ')' | ']' | '\n'
            )
        })
        .len();
    token.split_at(end)
}

fn redact_secret_values(line: &str) -> String {
    let mut out = line.to_string();
    for key in SECRET_KEYS {
        out = redact_key(&out, key);
    }
    out
}

/// Replace the value after `key = value` / `key: value` (JSON `"key": "value"`
/// included) with `[redacted]`. The value runs to the next delimiter
/// (whitespace, `,`, `;`, closing quote/brace/bracket) or end of line.
fn redact_key(line: &str, key: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut result = String::with_capacity(line.len());
    let mut i = 0;
    while i < line.len() {
        let rest_lower = &lower[i..];
        let Some(pos) = find_key_at(rest_lower, key) else {
            result.push_str(&line[i..]);
            break;
        };
        let key_start = i + pos;
        let mut j = key_start + key.len();
        // Optional closing quote (JSON key).
        if line[j..].starts_with('"') {
            j += 1;
        }
        let k = line[j..].find(|c: char| !c.is_whitespace());
        let mut j = match k {
            Some(offset) => j + offset,
            None => {
                result.push_str(&line[i..]);
                break;
            }
        };
        let next = line[j..].chars().next();
        if next != Some('=') && next != Some(':') {
            result.push_str(&line[i..key_start + 1]);
            i = key_start + 1;
            continue;
        }
        j += 1;
        while j < line.len() && line[j..].starts_with(char::is_whitespace) {
            j += 1;
        }
        let quoted = line[j..]
            .chars()
            .next()
            .is_some_and(|c| c == '"' || c == '\'');
        if quoted {
            j += 1;
        }
        result.push_str(&line[i..j]);
        result.push_str(REDACTED);
        let mut k = j;
        if quoted {
            while k < line.len() && !line[k..].starts_with(['"', '\'']) {
                k += 1;
            }
        } else {
            while k < line.len() && !line[k..].starts_with([' ', '\t', '\n', ',', ';', '}', ']']) {
                k += 1;
            }
        }
        i = k;
        // Skip (don't copy) the consumed value; keep the closing quote.
    }
    result
}

/// Case-insensitive whole-word search for `key` (not preceded/followed by an
/// identifier char, so `my_tokenizer` does not match `token`).
fn find_key_at(haystack_lower: &str, key: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(pos) = haystack_lower[from..].find(key) {
        let abs = from + pos;
        let before_ok = abs == 0
            || !haystack_lower[..abs]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        // A JSON key may be followed by a closing quote.
        let after = haystack_lower[abs + key.len()..].chars().next();
        let after_ok = after.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
        if before_ok && after_ok {
            return Some(abs);
        }
        from = abs + 1;
    }
    None
}

/// Today as `YYYY-MM-DD` (upstream `${shortdate}`) from the system clock.
pub fn today_ymd_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    ymd_from_unix(secs)
}

/// Unix seconds -> `YYYY-MM-DD` (Hinnant `civil_from_days`, no new deps).
pub fn ymd_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    y += i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ymd_known_dates() {
        assert_eq!(ymd_from_unix(0), "1970-01-01");
        assert_eq!(ymd_from_unix(1_704_067_200), "2024-01-01");
    }

    #[test]
    fn enabled_follows_settings() {
        let mut settings = AppSettings::default();
        assert!(enabled_from_settings(&settings));
        settings.gui_item.enable_log = false;
        assert!(!enabled_from_settings(&settings));
    }
}

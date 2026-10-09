//! Safe archive extraction.
//!
//! Both `zip` and `tar.gz` extraction enforce the same guarantees: no path may
//! escape the destination root (`..` or absolute), symlinks and hard links are
//! rejected outright, and total uncompressed bytes/entries are capped to stop
//! decompression bombs. Nothing is executed or mapped.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::error::UpdateError;

/// Extraction limits.
#[derive(Debug, Clone, Copy)]
pub struct UnpackLimits {
    /// Maximum total uncompressed bytes across all entries.
    pub max_total_bytes: u64,
    /// Maximum size of any single entry.
    pub max_entry_bytes: u64,
    /// Maximum number of entries.
    pub max_entries: usize,
}

impl Default for UnpackLimits {
    fn default() -> Self {
        Self {
            max_total_bytes: 1024 * 1024 * 1024,
            max_entry_bytes: 512 * 1024 * 1024,
            max_entries: 10_000,
        }
    }
}

/// Summary of a completed extraction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unpacked {
    pub entries: usize,
    pub bytes: u64,
    /// Relative paths written, in archive order.
    pub files: Vec<PathBuf>,
}

/// Validate an archive member path and resolve it under `root`.
///
/// Rejects absolute paths, `..` components and (on Windows) drive prefixes and
/// reserved names. Returns the safe joined destination.
pub fn safe_join(root: &Path, member: &str) -> Result<PathBuf, UpdateError> {
    if member.is_empty() {
        return Err(UpdateError::UnsafeArchivePath("empty member name".into()));
    }
    let normalized = member.replace('\\', "/");
    if normalized.starts_with('/') || looks_absolute_windows(&normalized) {
        return Err(UpdateError::UnsafeArchivePath(member.into()));
    }
    let mut out = root.to_path_buf();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => {
                if is_reserved_windows(part) {
                    return Err(UpdateError::UnsafeArchivePath(member.into()));
                }
                out.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(UpdateError::UnsafeArchivePath(member.into()));
            }
        }
    }
    if out == root {
        return Err(UpdateError::UnsafeArchivePath(member.into()));
    }
    Ok(out)
}

fn looks_absolute_windows(member: &str) -> bool {
    let bytes = member.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn is_reserved_windows(part: &std::ffi::OsStr) -> bool {
    let text = part.to_string_lossy();
    let text = text.to_ascii_uppercase();
    matches!(
        text.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// Extracting a zip archive into `dest`, enforcing `limits`.
pub fn safe_unpack_zip(
    archive_path: &Path,
    dest: &Path,
    limits: UnpackLimits,
) -> Result<Unpacked, UpdateError> {
    let file = std::fs::File::open(archive_path).map_err(|e| UpdateError::Io(e.to_string()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| UpdateError::UnsupportedArchive(e.to_string()))?;
    std::fs::create_dir_all(dest).map_err(|e| UpdateError::Io(e.to_string()))?;

    let mut summary = Unpacked::default();
    if archive.len() > limits.max_entries {
        return Err(UpdateError::UnsafeArchive(format!(
            "{} entries exceeds limit {}",
            archive.len(),
            limits.max_entries
        )));
    }
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| UpdateError::UnsupportedArchive(e.to_string()))?;
        let name = entry.name().to_string();
        if entry.is_dir() {
            let dir = safe_join(dest, &name)?;
            std::fs::create_dir_all(&dir).map_err(|e| UpdateError::Io(e.to_string()))?;
            continue;
        }
        if entry.is_symlink()
            || entry
                .unix_mode()
                .map(|m| m & 0o170000 == 0o120000)
                .unwrap_or(false)
        {
            return Err(UpdateError::UnsafeArchive(format!("symlink entry: {name}")));
        }
        if entry.size() > limits.max_entry_bytes {
            return Err(UpdateError::UnsafeArchive(format!(
                "entry {name} size {} exceeds limit {}",
                entry.size(),
                limits.max_entry_bytes
            )));
        }
        summary.bytes = summary
            .bytes
            .checked_add(entry.size())
            .ok_or_else(|| UpdateError::UnsafeArchive("size overflow".into()))?;
        if summary.bytes > limits.max_total_bytes {
            return Err(UpdateError::UnsafeArchive(format!(
                "total bytes exceed limit {}",
                limits.max_total_bytes
            )));
        }

        let output = safe_join(dest, &name)?;
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|e| UpdateError::Io(e.to_string()))?;
        }
        let mut writer =
            std::fs::File::create(&output).map_err(|e| UpdateError::Io(e.to_string()))?;
        // Copy with an independent cap: the declared size can lie.
        let mut limited = (&mut entry).take(limits.max_entry_bytes + 1);
        let written =
            std::io::copy(&mut limited, &mut writer).map_err(|e| UpdateError::Io(e.to_string()))?;
        if written > limits.max_entry_bytes {
            let _ = std::fs::remove_file(&output);
            return Err(UpdateError::UnsafeArchive(format!(
                "entry {name} wrote past limit"
            )));
        }
        summary.entries += 1;
        summary.files.push(output);
    }
    Ok(summary)
}

/// Extract a gzip-compressed tar archive into `dest`, enforcing `limits`.
///
/// A minimal USTAR/GNU header reader is used instead of a `tar` dependency:
/// only regular files and directories are accepted, everything else
/// (symlinks, hard links, devices, FIFOs) is rejected.
pub fn safe_unpack_targz(
    archive_path: &Path,
    dest: &Path,
    limits: UnpackLimits,
) -> Result<Unpacked, UpdateError> {
    let file = std::fs::File::open(archive_path).map_err(|e| UpdateError::Io(e.to_string()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut reader = std::io::BufReader::new(decoder);
    std::fs::create_dir_all(dest).map_err(|e| UpdateError::Io(e.to_string()))?;

    let mut summary = Unpacked::default();
    let mut header = [0u8; 512];
    loop {
        if !read_exact_or_eof(&mut reader, &mut header)
            .map_err(|e| UpdateError::Io(e.to_string()))?
        {
            break;
        }
        if header.iter().all(|b| *b == 0) {
            // End-of-archive marker; trailing blocks may follow.
            break;
        }
        let name = tar_string(&header[0..100]);
        let size_field = tar_string(&header[124..136]);
        let typeflag = header[156];
        let prefix = tar_string(&header[345..500]);
        let member = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if member.is_empty() {
            return Err(UpdateError::UnsafeArchive("tar entry without name".into()));
        }
        let size = if size_field.trim().is_empty() {
            0u64
        } else {
            u64::from_str_radix(size_field.trim(), 8)
                .map_err(|_| UpdateError::UnsupportedArchive("bad tar size field".into()))?
        };

        match typeflag {
            b'0' | 0 => {
                summary.entries += 1;
                if summary.entries > limits.max_entries {
                    return Err(UpdateError::UnsafeArchive("too many entries".into()));
                }
                if size > limits.max_entry_bytes {
                    return Err(UpdateError::UnsafeArchive(format!(
                        "entry {member} size {size} exceeds limit"
                    )));
                }
                summary.bytes = summary
                    .bytes
                    .checked_add(size)
                    .ok_or_else(|| UpdateError::UnsafeArchive("size overflow".into()))?;
                if summary.bytes > limits.max_total_bytes {
                    return Err(UpdateError::UnsafeArchive(
                        "total bytes exceed limit".into(),
                    ));
                }
                let output = safe_join(dest, &member)?;
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| UpdateError::Io(e.to_string()))?;
                }
                let mut writer =
                    std::fs::File::create(&output).map_err(|e| UpdateError::Io(e.to_string()))?;
                let mut limited = (&mut reader).take(size);
                let written = std::io::copy(&mut limited, &mut writer)
                    .map_err(|e| UpdateError::Io(e.to_string()))?;
                if written != size {
                    return Err(UpdateError::UnsupportedArchive(
                        "truncated tar entry".into(),
                    ));
                }
                pad_tar(&mut reader, size).map_err(|e| UpdateError::Io(e.to_string()))?;
                summary.files.push(output);
            }
            b'5' => {
                let dir = safe_join(dest, &member)?;
                std::fs::create_dir_all(&dir).map_err(|e| UpdateError::Io(e.to_string()))?;
            }
            b'1' | b'2' => {
                return Err(UpdateError::UnsafeArchive(format!("link entry: {member}")));
            }
            // Pax/GNU metadata or other special entries: skip their payload.
            _ => {
                pad_tar(&mut reader, size).map_err(|e| UpdateError::Io(e.to_string()))?;
            }
        }
    }
    Ok(summary)
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..])? {
            0 => return Ok(false),
            n => filled += n,
        }
    }
    Ok(true)
}

fn tar_string(raw: &[u8]) -> String {
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).trim().to_string()
}

fn pad_tar<R: Read>(reader: &mut R, size: u64) -> std::io::Result<()> {
    let remainder = (512 - (size % 512)) % 512;
    let mut skip = vec![0u8; remainder as usize];
    let mut filled = 0;
    while filled < skip.len() {
        match reader.read(&mut skip[filled..])? {
            0 => return Ok(()),
            n => filled += n,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_rejects_escaping_paths() {
        let root = Path::new("/tmp/root");
        assert!(matches!(
            safe_join(root, "../evil"),
            Err(UpdateError::UnsafeArchivePath(_))
        ));
        assert!(matches!(
            safe_join(root, "a/../../evil"),
            Err(UpdateError::UnsafeArchivePath(_))
        ));
        assert!(matches!(
            safe_join(root, "/etc/passwd"),
            Err(UpdateError::UnsafeArchivePath(_))
        ));
        assert!(matches!(
            safe_join(root, "C:/windows/system32"),
            Err(UpdateError::UnsafeArchivePath(_))
        ));
        assert!(matches!(
            safe_join(root, "..\\evil"),
            Err(UpdateError::UnsafeArchivePath(_))
        ));
    }

    #[test]
    fn safe_join_allows_normal_paths() {
        let root = Path::new("/tmp/root");
        assert_eq!(
            safe_join(root, "bin/xray").unwrap(),
            Path::new("/tmp/root/bin/xray")
        );
        assert_eq!(
            safe_join(root, "./a/b").unwrap(),
            Path::new("/tmp/root/a/b")
        );
    }

    #[test]
    fn safe_join_normalizes_backslashes() {
        let root = Path::new("/tmp/root");
        assert_eq!(
            safe_join(root, "bin\\xray.exe").unwrap(),
            Path::new("/tmp/root/bin/xray.exe")
        );
    }
}

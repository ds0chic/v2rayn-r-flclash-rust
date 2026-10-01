//! Archive builders for T16 tests.
//!
//! All builders write to caller-provided temp paths. They intentionally include
//! malicious entries (traversal, symlink, oversized) so the extractor's guards
//! are exercised with real bytes.

#![allow(dead_code)]

use std::io::Write;
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// A regular file entry.
pub struct ZipEntry<'a> {
    pub name: &'a str,
    pub data: &'a [u8],
    /// Store as a symlink whose content is the target path.
    pub symlink_target: Option<&'a str>,
}

impl<'a> ZipEntry<'a> {
    pub fn file(name: &'a str, data: &'a [u8]) -> Self {
        Self {
            name,
            data,
            symlink_target: None,
        }
    }

    pub fn symlink(name: &'a str, target: &'a str) -> Self {
        Self {
            name,
            data: target.as_bytes(),
            symlink_target: Some(target),
        }
    }
}

/// Write a zip archive to `path` with the given entries.
pub fn write_zip(path: &Path, entries: &[ZipEntry]) {
    let file = std::fs::File::create(path).expect("create zip");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for entry in entries {
        if let Some(target) = entry.symlink_target {
            writer
                .add_symlink(entry.name, target, options)
                .expect("add symlink");
        } else {
            writer.start_file(entry.name, options).expect("start file");
            writer.write_all(entry.data).expect("write data");
        }
    }
    writer.finish().expect("finish zip");
}

/// Write a plain (uncompressed) tar to `path` from `(name, data)` pairs.
///
/// A minimal USTAR writer; only regular files are produced. Symlink entries are
/// written by passing `typeflag` directly.
pub fn write_tar(path: &Path, entries: &[(&str, &[u8])], gzip: bool) {
    let mut raw = Vec::new();
    for (name, data) in entries {
        append_tar_entry(&mut raw, name, data, b'0');
    }
    // Two zero blocks terminate the archive.
    raw.extend_from_slice(&[0u8; 1024]);

    let bytes = if gzip {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&raw).expect("gzip");
        encoder.finish().expect("gzip finish")
    } else {
        raw
    };
    std::fs::write(path, bytes).expect("write tar");
}

/// Write a tar with an explicit typeflag (e.g. `b'2'` for symlink).
pub fn write_tar_typed(path: &Path, entries: &[(&str, &[u8], u8)], gzip: bool) {
    let mut raw = Vec::new();
    for (name, data, typeflag) in entries {
        append_tar_entry(&mut raw, name, data, *typeflag);
    }
    raw.extend_from_slice(&[0u8; 1024]);
    let bytes = if gzip {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&raw).expect("gzip");
        encoder.finish().expect("gzip finish")
    } else {
        raw
    };
    std::fs::write(path, bytes).expect("write tar");
}

fn append_tar_entry(out: &mut Vec<u8>, name: &str, data: &[u8], typeflag: u8) {
    let mut header = [0u8; 512];
    let name_bytes = name.as_bytes();
    let len = name_bytes.len().min(100);
    header[..len].copy_from_slice(&name_bytes[..len]);
    // mode 0644
    header[100..108].copy_from_slice(b"0000644\0");
    // uid/gid 0
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    // size in octal
    let size_field = format!("{:011o}\0", data.len());
    header[124..136].copy_from_slice(size_field.as_bytes());
    // mtime
    header[136..148].copy_from_slice(b"00000000000\0");
    // checksum placeholder spaces
    header[148..156].copy_from_slice(b"        ");
    header[156] = typeflag;
    // ustar magic
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    // checksum
    let checksum: u32 = header.iter().map(|b| *b as u32).sum();
    let checksum_field = format!("{:06o}\0 ", checksum);
    header[148..156].copy_from_slice(checksum_field.as_bytes());

    out.extend_from_slice(&header);
    out.extend_from_slice(data);
    let remainder = (512 - (data.len() % 512)) % 512;
    out.extend(std::iter::repeat_n(0u8, remainder));
}

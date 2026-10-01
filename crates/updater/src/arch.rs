//! Platform/architecture vocabulary and read-only binary header parsing.
//!
//! The asset selector keys on the same OS×arch units the project ships
//! (`windows-x64/x86/arm64`, `linux-x64/arm64`, `macos-x64/arm64`). The header
//! parser only reads magic bytes; it never maps or executes the file.

use crate::error::UpdateError;
use crate::semver::Semver;

/// Operating system family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Windows,
    Linux,
    Macos,
}

impl Os {
    pub fn as_str(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::Linux => "linux",
            Os::Macos => "macos",
        }
    }
}

/// CPU architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformArch {
    X64,
    X86,
    Arm64,
}

impl PlatformArch {
    pub fn as_str(self) -> &'static str {
        match self {
            PlatformArch::X64 => "x64",
            PlatformArch::X86 => "x86",
            PlatformArch::Arm64 => "arm64",
        }
    }
}

/// A build target an update asset can satisfy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostTarget {
    pub os: Os,
    pub arch: PlatformArch,
}

impl HostTarget {
    pub fn new(os: Os, arch: PlatformArch) -> Self {
        Self { os, arch }
    }

    pub fn key(self) -> String {
        format!("{}-{}", self.os.as_str(), self.arch.as_str())
    }
}

/// The target this binary was compiled for, or `None` on a unit outside the
/// project's six-unit delivery matrix (e.g. riscv64).
pub fn detect_target() -> Option<HostTarget> {
    let arch = if cfg!(target_arch = "x86_64") {
        PlatformArch::X64
    } else if cfg!(target_arch = "aarch64") {
        PlatformArch::Arm64
    } else if cfg!(target_arch = "x86") {
        PlatformArch::X86
    } else {
        return None;
    };
    let os = if cfg!(target_os = "windows") {
        Os::Windows
    } else if cfg!(target_os = "linux") {
        Os::Linux
    } else if cfg!(target_os = "macos") {
        Os::Macos
    } else {
        return None;
    };
    Some(HostTarget::new(os, arch))
}

/// Normalize a host triple string (`x86_64-pc-windows-msvc`, `aarch64-apple-darwin`).
pub fn parse_triple(triple: &str) -> Option<HostTarget> {
    let triple = triple.to_ascii_lowercase();
    let arch = if triple.starts_with("x86_64") || triple.starts_with("amd64") {
        PlatformArch::X64
    } else if triple.starts_with("aarch64") || triple.starts_with("arm64") {
        PlatformArch::Arm64
    } else if triple.starts_with("i686") || triple.starts_with("i586") || triple.starts_with("x86-")
    {
        PlatformArch::X86
    } else {
        return None;
    };
    let os = if triple.contains("windows") {
        Os::Windows
    } else if triple.contains("darwin") || triple.contains("macos") {
        Os::Macos
    } else if triple.contains("linux") {
        Os::Linux
    } else {
        return None;
    };
    Some(HostTarget::new(os, arch))
}

/// A binary's format and machine architecture, derived from its header only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFormat {
    Pe,
    Elf,
    MachO,
}

/// Result of a read-only binary header parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryArch {
    pub format: BinaryFormat,
    /// Normalized architecture, or `None` if the format/machine is unknown.
    pub arch: Option<PlatformArch>,
    /// Raw machine value for diagnostics (PE machine / ELF e_machine / Mach-O cpu).
    pub machine: u32,
}

impl BinaryArch {
    /// Whether this binary matches `target`'s OS family and architecture.
    pub fn matches(&self, target: HostTarget) -> bool {
        let os_ok = matches!(
            (self.format, target.os),
            (BinaryFormat::Pe, Os::Windows)
                | (BinaryFormat::Elf, Os::Linux)
                | (BinaryFormat::MachO, Os::Macos)
        );
        os_ok && self.arch == Some(target.arch)
    }

    /// Target implied by this binary, if both format and arch are known.
    pub fn implied_target(&self) -> Option<HostTarget> {
        let os = match self.format {
            BinaryFormat::Pe => Os::Windows,
            BinaryFormat::Elf => Os::Linux,
            BinaryFormat::MachO => Os::Macos,
        };
        self.arch.map(|arch| HostTarget::new(os, arch))
    }
}

const PE_MACHINE_X64: u32 = 0x8664;
const PE_MACHINE_X86: u32 = 0x014c;
const PE_MACHINE_ARM64: u32 = 0xaa64;

const ELF_MACHINE_X64: u32 = 62;
const ELF_MACHINE_X86: u32 = 3;
const ELF_MACHINE_ARM64: u32 = 183;

const MACHO_CPU_X64: u32 = 0x0100_0007;
const MACHO_CPU_X86: u32 = 7;
const MACHO_CPU_ARM64: u32 = 0x0100_000c;

fn arch_from_machine(format: BinaryFormat, machine: u32) -> Option<PlatformArch> {
    match format {
        BinaryFormat::Pe => match machine {
            PE_MACHINE_X64 => Some(PlatformArch::X64),
            PE_MACHINE_X86 => Some(PlatformArch::X86),
            PE_MACHINE_ARM64 => Some(PlatformArch::Arm64),
            _ => None,
        },
        BinaryFormat::Elf => match machine {
            ELF_MACHINE_X64 => Some(PlatformArch::X64),
            ELF_MACHINE_X86 => Some(PlatformArch::X86),
            ELF_MACHINE_ARM64 => Some(PlatformArch::Arm64),
            _ => None,
        },
        BinaryFormat::MachO => match machine {
            MACHO_CPU_X64 => Some(PlatformArch::X64),
            MACHO_CPU_X86 => Some(PlatformArch::X86),
            MACHO_CPU_ARM64 => Some(PlatformArch::Arm64),
            _ => None,
        },
    }
}

fn le_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
}

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *bytes.get(at)?,
        *bytes.get(at + 1)?,
        *bytes.get(at + 2)?,
        *bytes.get(at + 3)?,
    ]))
}

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *bytes.get(at)?,
        *bytes.get(at + 1)?,
        *bytes.get(at + 2)?,
        *bytes.get(at + 3)?,
    ]))
}

/// Parse a PE/ELF/Mach-O header without executing or mapping it.
pub fn parse_binary_arch(bytes: &[u8]) -> Result<BinaryArch, UpdateError> {
    if bytes.len() >= 2 && &bytes[..2] == b"MZ" {
        let pe_offset = le_u32(bytes, 0x3c)
            .ok_or_else(|| UpdateError::InvalidMetadata("truncated PE header".into()))?
            as usize;
        if bytes.get(pe_offset..pe_offset + 4) != Some(b"PE\0\0") {
            return Err(UpdateError::InvalidMetadata("PE signature missing".into()));
        }
        let machine = le_u16(bytes, pe_offset + 4)
            .ok_or_else(|| UpdateError::InvalidMetadata("truncated PE COFF header".into()))?
            as u32;
        return Ok(BinaryArch {
            format: BinaryFormat::Pe,
            arch: arch_from_machine(BinaryFormat::Pe, machine),
            machine,
        });
    }

    if bytes.starts_with(&[0x7f, b'E', b'L', b'F']) {
        let class = *bytes
            .get(4)
            .ok_or_else(|| UpdateError::InvalidMetadata("truncated ELF header".into()))?;
        let data = *bytes
            .get(5)
            .ok_or_else(|| UpdateError::InvalidMetadata("truncated ELF header".into()))?;
        // class 2 == ELFCLASS64, data 1 == little endian.
        if class != 2 || data != 1 {
            return Ok(BinaryArch {
                format: BinaryFormat::Elf,
                arch: None,
                machine: 0,
            });
        }
        let machine = le_u16(bytes, 18)
            .ok_or_else(|| UpdateError::InvalidMetadata("truncated ELF header".into()))?
            as u32;
        return Ok(BinaryArch {
            format: BinaryFormat::Elf,
            arch: arch_from_machine(BinaryFormat::Elf, machine),
            machine,
        });
    }

    // Mach-O magic is endianness-dependent; match both byte orders and pick
    // the matching cputype read order.
    if bytes.len() >= 8 {
        let magic_le = le_u32(bytes, 0).unwrap();
        let magic_be = be_u32(bytes, 0).unwrap();
        if is_macho_magic(magic_le) {
            let cputype = le_u32(bytes, 4).unwrap_or(0);
            return Ok(BinaryArch {
                format: BinaryFormat::MachO,
                arch: arch_from_machine(BinaryFormat::MachO, cputype),
                machine: cputype,
            });
        }
        if is_macho_magic(magic_be) {
            let cputype = be_u32(bytes, 4).unwrap_or(0);
            return Ok(BinaryArch {
                format: BinaryFormat::MachO,
                arch: arch_from_machine(BinaryFormat::MachO, cputype),
                machine: cputype,
            });
        }
    }

    Err(UpdateError::InvalidMetadata(
        "unrecognized binary format".into(),
    ))
}

fn is_macho_magic(magic: u32) -> bool {
    matches!(magic, 0xfeed_facf | 0xfeed_face | 0xcafe_babe | 0xcafe_babf)
}

/// A convenience wrapper: does `bytes` match `target`?
pub fn binary_matches(bytes: &[u8], target: HostTarget) -> Result<bool, UpdateError> {
    Ok(parse_binary_arch(bytes)?.matches(target))
}

/// Version bound helper used by channel/capability rules.
pub fn within_range(version: &Semver, max: &Semver) -> bool {
    version <= max
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pe(machine: u16) -> Vec<u8> {
        let mut buf = vec![0u8; 0x80];
        buf[..2].copy_from_slice(b"MZ");
        buf[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        buf[0x40..0x44].copy_from_slice(b"PE\0\0");
        buf[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
        buf
    }

    fn elf64(machine: u16, little_endian: bool) -> Vec<u8> {
        let mut buf = vec![0u8; 0x40];
        buf[..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        buf[4] = 2;
        buf[5] = if little_endian { 1 } else { 2 };
        let bytes = if little_endian {
            machine.to_le_bytes()
        } else {
            machine.to_be_bytes()
        };
        buf[18..20].copy_from_slice(&bytes);
        buf
    }

    fn macho(cputype: u32) -> Vec<u8> {
        let mut buf = vec![0u8; 0x20];
        buf[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
        buf[4..8].copy_from_slice(&cputype.to_le_bytes());
        buf
    }

    #[test]
    fn pe_machines() {
        let x64 = parse_binary_arch(&pe(PE_MACHINE_X64 as u16)).unwrap();
        assert_eq!(x64.format, BinaryFormat::Pe);
        assert_eq!(x64.arch, Some(PlatformArch::X64));
        assert!(x64.matches(HostTarget::new(Os::Windows, PlatformArch::X64)));
        assert!(!x64.matches(HostTarget::new(Os::Linux, PlatformArch::X64)));

        assert_eq!(
            parse_binary_arch(&pe(PE_MACHINE_X86 as u16)).unwrap().arch,
            Some(PlatformArch::X86)
        );
        assert_eq!(
            parse_binary_arch(&pe(PE_MACHINE_ARM64 as u16))
                .unwrap()
                .arch,
            Some(PlatformArch::Arm64)
        );
        assert_eq!(parse_binary_arch(&pe(0x1234)).unwrap().arch, None);
    }

    #[test]
    fn elf_machines() {
        let x64 = parse_binary_arch(&elf64(ELF_MACHINE_X64 as u16, true)).unwrap();
        assert_eq!(x64.format, BinaryFormat::Elf);
        assert_eq!(x64.arch, Some(PlatformArch::X64));
        assert!(x64.matches(HostTarget::new(Os::Linux, PlatformArch::X64)));

        assert_eq!(
            parse_binary_arch(&elf64(ELF_MACHINE_ARM64 as u16, true))
                .unwrap()
                .arch,
            Some(PlatformArch::Arm64)
        );
        // big-endian / 32-bit ELF is recognized but not mapped.
        assert_eq!(
            parse_binary_arch(&elf64(ELF_MACHINE_X64 as u16, false))
                .unwrap()
                .arch,
            None
        );
    }

    #[test]
    fn macho_machines() {
        let x64 = parse_binary_arch(&macho(MACHO_CPU_X64)).unwrap();
        assert_eq!(x64.format, BinaryFormat::MachO);
        assert_eq!(x64.arch, Some(PlatformArch::X64));
        assert!(x64.matches(HostTarget::new(Os::Macos, PlatformArch::X64)));

        assert_eq!(
            parse_binary_arch(&macho(MACHO_CPU_ARM64)).unwrap().arch,
            Some(PlatformArch::Arm64)
        );
    }

    #[test]
    fn unknown_format_is_rejected() {
        assert!(matches!(
            parse_binary_arch(b"plain text file"),
            Err(UpdateError::InvalidMetadata(_))
        ));
    }

    #[test]
    fn truncated_pe_is_rejected() {
        assert!(matches!(
            parse_binary_arch(b"MZ\0\0"),
            Err(UpdateError::InvalidMetadata(_))
        ));
    }

    #[test]
    fn mismatched_arch_compares_correctly() {
        let bytes = pe(PE_MACHINE_ARM64 as u16);
        assert!(!binary_matches(&bytes, HostTarget::new(Os::Windows, PlatformArch::X64)).unwrap());
        assert!(binary_matches(&bytes, HostTarget::new(Os::Windows, PlatformArch::Arm64)).unwrap());
    }

    #[test]
    fn triple_parsing() {
        assert_eq!(
            parse_triple("x86_64-pc-windows-msvc"),
            Some(HostTarget::new(Os::Windows, PlatformArch::X64))
        );
        assert_eq!(
            parse_triple("aarch64-apple-darwin"),
            Some(HostTarget::new(Os::Macos, PlatformArch::Arm64))
        );
        assert_eq!(
            parse_triple("x86_64-unknown-linux-gnu"),
            Some(HostTarget::new(Os::Linux, PlatformArch::X64))
        );
        assert_eq!(parse_triple("riscv64gc-unknown-linux-gnu"), None);
    }

    #[test]
    fn implied_target_roundtrip() {
        let bytes = pe(PE_MACHINE_X64 as u16);
        assert_eq!(
            parse_binary_arch(&bytes).unwrap().implied_target(),
            Some(HostTarget::new(Os::Windows, PlatformArch::X64))
        );
    }
}

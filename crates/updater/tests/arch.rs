//! PE/ELF/Mach-O read-only header parsing.

use updater::arch::{BinaryFormat, HostTarget, Os, PlatformArch};
use updater::{binary_matches, parse_binary_arch, UpdateError};

fn pe(machine: u16) -> Vec<u8> {
    let mut buf = vec![0u8; 0x80];
    buf[..2].copy_from_slice(b"MZ");
    buf[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    buf[0x40..0x44].copy_from_slice(b"PE\0\0");
    buf[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
    buf
}

fn elf64(machine: u16) -> Vec<u8> {
    let mut buf = vec![0u8; 0x40];
    buf[..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    buf[4] = 2;
    buf[5] = 1;
    buf[18..20].copy_from_slice(&machine.to_le_bytes());
    buf
}

fn macho(cputype: u32) -> Vec<u8> {
    let mut buf = vec![0u8; 0x20];
    buf[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
    buf[4..8].copy_from_slice(&cputype.to_le_bytes());
    buf
}

#[test]
fn parses_pe_x64() {
    let parsed = parse_binary_arch(&pe(0x8664)).unwrap();
    assert_eq!(parsed.format, BinaryFormat::Pe);
    assert_eq!(parsed.arch, Some(PlatformArch::X64));
    assert_eq!(
        parsed.implied_target(),
        Some(HostTarget::new(Os::Windows, PlatformArch::X64))
    );
}

#[test]
fn parses_pe_x86_and_arm64() {
    assert_eq!(
        parse_binary_arch(&pe(0x014c)).unwrap().arch,
        Some(PlatformArch::X86)
    );
    assert_eq!(
        parse_binary_arch(&pe(0xaa64)).unwrap().arch,
        Some(PlatformArch::Arm64)
    );
}

#[test]
fn unknown_pe_machine_null_arch() {
    assert_eq!(parse_binary_arch(&pe(0x1234)).unwrap().arch, None);
}

#[test]
fn truncated_pe_rejected() {
    assert!(matches!(
        parse_binary_arch(b"MZ\0\0"),
        Err(UpdateError::InvalidMetadata(_))
    ));
}

#[test]
fn parses_elf64_x64_and_arm64() {
    let x64 = parse_binary_arch(&elf64(62)).unwrap();
    assert_eq!(x64.format, BinaryFormat::Elf);
    assert_eq!(x64.arch, Some(PlatformArch::X64));
    assert_eq!(
        x64.implied_target(),
        Some(HostTarget::new(Os::Linux, PlatformArch::X64))
    );
    assert_eq!(
        parse_binary_arch(&elf64(183)).unwrap().arch,
        Some(PlatformArch::Arm64)
    );
    assert_eq!(
        parse_binary_arch(&elf64(3)).unwrap().arch,
        Some(PlatformArch::X86)
    );
}

#[test]
fn big_endian_elf_not_mapped() {
    let mut buf = elf64(62);
    buf[5] = 2;
    assert_eq!(parse_binary_arch(&buf).unwrap().arch, None);
}

#[test]
fn parses_macho_x64_and_arm64() {
    let x64 = parse_binary_arch(&macho(0x0100_0007)).unwrap();
    assert_eq!(x64.format, BinaryFormat::MachO);
    assert_eq!(x64.arch, Some(PlatformArch::X64));
    assert_eq!(
        x64.implied_target(),
        Some(HostTarget::new(Os::Macos, PlatformArch::X64))
    );
    assert_eq!(
        parse_binary_arch(&macho(0x0100_000c)).unwrap().arch,
        Some(PlatformArch::Arm64)
    );
}

#[test]
fn unknown_format_rejected() {
    assert!(matches!(
        parse_binary_arch(b"#!/bin/sh\necho hi"),
        Err(UpdateError::InvalidMetadata(_))
    ));
}

#[test]
fn binary_matches_target() {
    let win64 = HostTarget::new(Os::Windows, PlatformArch::X64);
    assert!(binary_matches(&pe(0x8664), win64).unwrap());
    assert!(!binary_matches(&pe(0xaa64), win64).unwrap());
    assert!(!binary_matches(&elf64(62), win64).unwrap());
}

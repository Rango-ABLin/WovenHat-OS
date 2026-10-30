// ELF Binary Loader and Program Header Parser for WovenHat-OS
extern crate alloc;
use alloc::vec::Vec;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Mapper, Size4KiB};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElfError {
    InvalidMagic,
    InvalidHeader,
    UnsupportedArchitecture,
    SegmentMappingFailed,
}

pub struct ElfLoader;

impl ElfLoader {
    pub fn parse_and_load(
        elf_bytes: &[u8],
        _page_table: &mut impl Mapper<Size4KiB>,
    ) -> Result<VirtAddr, ElfError> {
        if elf_bytes.len() < 64 || &elf_bytes[0..4] != b"\x7FELF" {
            return Err(ElfError::InvalidMagic);
        }

        if elf_bytes[4] != 2 {
            let mut audit = crate::audit::BoundedAuditRing::new();
            audit.log_event(0, crate::audit::AuditAction::CapabilityCheckDenied, crate::capabilities::CapabilityType::MemoryInspect, false, 0);
            return Err(ElfError::UnsupportedArchitecture);
        }

        let entry_point_u64 = u64::from_le_bytes(
            elf_bytes[24..32].try_into().map_err(|_| ElfError::InvalidHeader)?
        );

        let phoff = u64::from_le_bytes(
            elf_bytes[32..40].try_into().map_err(|_| ElfError::InvalidHeader)?
        ) as usize;

        let phentsize = u16::from_le_bytes(
            elf_bytes[54..56].try_into().map_err(|_| ElfError::InvalidHeader)?
        ) as usize;
        let phnum = u16::from_le_bytes(
            elf_bytes[56..58].try_into().map_err(|_| ElfError::InvalidHeader)?
        ) as usize;

        for i in 0..phnum {
            let ph_offset = phoff + i * phentsize;
            if ph_offset + 56 > elf_bytes.len() {
                break;
            }

            let p_type = u32::from_le_bytes(elf_bytes[ph_offset..ph_offset+4].try_into().unwrap());
            if p_type == 1 {
                let _p_flags = u32::from_le_bytes(elf_bytes[ph_offset+4..ph_offset+8].try_into().unwrap());
                let p_offset = u64::from_le_bytes(elf_bytes[ph_offset+8..ph_offset+16].try_into().unwrap()) as usize;
                let p_vaddr = u64::from_le_bytes(elf_bytes[ph_offset+16..ph_offset+24].try_into().unwrap());
                let p_filesz = u64::from_le_bytes(elf_bytes[ph_offset+32..ph_offset+40].try_into().unwrap()) as usize;
                let _p_memsz = u64::from_le_bytes(elf_bytes[ph_offset+40..ph_offset+48].try_into().unwrap()) as usize;

                if p_offset + p_filesz > elf_bytes.len() {
                    return Err(ElfError::InvalidHeader);
                }

                let _target_vaddr = VirtAddr::new(p_vaddr);
            }
        }

        Ok(VirtAddr::new(entry_point_u64))
    }
}

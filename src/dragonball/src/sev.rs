use once_cell::sync::Lazy;
use raw_cpuid::{cpuid, CpuIdResult};

/// Ref: AMD64 Architecture Programmer’s Manual, Volume 3: General-Purpose and
/// System Instructions, 24594
/// E.4.17 Function 8000_001Fh—Encrypted Memory Capabilities
/// https://www.amd.com/system/files/TechDocs/24594.pdf
pub(crate) struct CpuIdAmdEmc(CpuIdResult);

#[allow(non_snake_case)]
impl CpuIdAmdEmc {
    pub(crate) fn SME(&self) -> bool {
        self.0.eax & (1 << 0) != 0
    }
    pub(crate) fn SEV(&self) -> bool {
        self.0.eax & (1 << 1) != 0
    }
    pub(crate) fn SEV_ES(&self) -> bool {
        self.0.eax & (1 << 3) != 0
    }
    pub(crate) fn SEV_SNP(&self) -> bool {
        self.0.eax & (1 << 4) != 0
    }
    pub(crate) fn cbitpos(&self) -> u32 {
        self.0.ebx & 0b11_1111
    }
    pub(crate) fn phys_addr_reduction(&self) -> u32 {
        (self.0.ebx >> 6) & 0b11_1111
    }
    pub(crate) fn num_encrypted_guests(&self) -> u32 {
        self.0.ecx
    }
}

pub(crate) static HOST_CPUID_AMD_EMC: Lazy<CpuIdAmdEmc> = Lazy::new(|| {
    let registers = cpuid!(0x8000_001f, 0);
    CpuIdAmdEmc(registers)
});

/// A module containing AMD SEV and SEV-ES related facilities, SEV-SNP not included.
#[allow(clippy::module_inception)]
pub mod sev {
    use sev::launch::sev::Secret;

    /// SEV Secret with the guest physical address where it will be injected into.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SecretWithGpa {
        /// A packet containing secret information to be injected into the guest.
        pub secret: Secret,
        /// Guest physical address where the secret will be injected into.
        /// If is None, the address will be determined by VMM.
        pub gpa: Option<usize>,
    }

    /// When `paused` is set to true in [`SevStart`](crate::vm::SevStart), the VM
    /// will pause after starting, waiting for the injection of secrets.
    /// You can include zero or more secrets in the `secrets` field of this struct.
    /// If `resume_vm` is set to true, the VM will resume after the secrets are
    /// injected, and no further injection is allowed afterwards; otherwise, it
    /// will remain paused and secrets can continue to be injected. 
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct SevSecretsInjection {
        /// Zero or more secrets.
        pub secrets: Vec<SecretWithGpa>,
        /// Whether the VM will resume after injection.
        pub resume_vm: bool,
    }

    /// TD shim related facilities
    pub mod tdshim {
        use std::{convert::TryInto, fs::File, os::unix::prelude::FileExt};
        use thiserror::Error;

        /// TDVF related errors.
        #[derive(Error, Debug)]
        pub enum ParseOvmfTableError {
            /// Failed to read data at the given offset in firmware file.
            #[error("Failed to read data at the given offset in firmware file: {0}")]
            ReadOffset(#[source] std::io::Error),
            /// Abnormal OVMF table format
            #[error("Abnormal OVMF table format")]
            InvalidOvmfTableFormat,
        }

        const OVMF_TABLE_FOOTER_GUID: [u8; 16] = [
            222, 130, 181, 150, 178, 31, 247, 69, 186, 234, 163, 102, 197, 90, 8, 45,
        ];
        const SEV_SECRET_BLOCK_GUID: [u8; 16] = [
            97, 179, 46, 76, 155, 125, 195, 76, 128, 129, 18, 124, 144, 211, 210, 148,
        ];
        const SEV_HASH_TABLE_RV_GUID: [u8; 16] = [
            31, 55, 85, 114, 59, 58, 4, 75, 146, 123, 29, 166, 239, 168, 212, 84,
        ];

        /// Ref: https://qemu.readthedocs.io/en/latest/specs/sev-guest-firmware.html
        ///
        /// Be careful of integer and buffer overflow.
        ///
        /// ```text
        /// +-- Entries (zero or more) -----------+
        /// |                entry data: any bytes|
        /// |         2B     entry length         |
        /// |        16B     entry GUID           |
        /// +-------------------------------------+
        /// EOF-50    2B     OVMF table length
        /// EOF-48   16B     OVMF footer GUID
        /// End of File
        /// ```
        pub(crate) fn parse_ovmf_table(
            firmware_file: &File,
        ) -> Result<(SevSecretBlockGpa, SevHashesTableGpa), ParseOvmfTableError> {
            let firmware_size = firmware_file
                .metadata()
                .map_err(ParseOvmfTableError::ReadOffset)?
                .len();

            let mut f = FirmwareFileReader {
                firmware_file,
                offset: firmware_size - 32,
                buf: [0_u8; 16],
            };
            struct FirmwareFileReader<'a> {
                firmware_file: &'a File,
                offset: u64,
                buf: [u8; 16],
            }
            impl<'a> FirmwareFileReader<'a> {
                /// Move backward and read `len` bytes.
                fn read(&mut self, len: usize) -> Result<&[u8], ParseOvmfTableError> {
                    debug_assert!(len <= 16);

                    self.offset -= len as u64;
                    let ret = unsafe { std::slice::from_raw_parts_mut(self.buf.as_mut_ptr(), len) };
                    self.firmware_file
                        .read_exact_at(ret, self.offset)
                        .map_err(ParseOvmfTableError::ReadOffset)?;

                    Ok(ret)
                }
                fn read_u16(&mut self) -> Result<u16, ParseOvmfTableError> {
                    Ok(u16::from_le_bytes(self.read(2)?.try_into().unwrap()))
                }
                fn read_u32(&mut self) -> Result<u32, ParseOvmfTableError> {
                    Ok(u32::from_le_bytes(self.read(4)?.try_into().unwrap()))
                }
            }

            if firmware_size < 32 + 16 + 2 {
                return Err(ParseOvmfTableError::InvalidOvmfTableFormat);
            }

            let footer_guid = f.read(16)?;
            if footer_guid != OVMF_TABLE_FOOTER_GUID {
                return Err(ParseOvmfTableError::InvalidOvmfTableFormat);
            }

            let table_len = f.read_u16()? as u64;
            if table_len > firmware_size - 32 {
                return Err(ParseOvmfTableError::InvalidOvmfTableFormat);
            }
            let mut bytes_left = table_len - 16 - 2;

            let mut entries_not_found_yet = 2;
            let mut sev_secret_block: Option<SevSecretBlockGpa> = None;
            let mut sev_hashes_table: Option<SevHashesTableGpa> = None;

            while bytes_left >= 16 + 2 && entries_not_found_yet > 0 {
                let guid: [u8; 16] = f.read(16)?.try_into().unwrap();
                let entry_len = f.read_u16()? as u64;

                bytes_left = bytes_left
                    .checked_sub(entry_len)
                    .ok_or(ParseOvmfTableError::InvalidOvmfTableFormat)?;

                match guid {
                    SEV_SECRET_BLOCK_GUID => {
                        if entry_len != 0x1a || sev_secret_block.is_some() {
                            return Err(ParseOvmfTableError::InvalidOvmfTableFormat);
                        }
                        let size = f.read_u32()?;
                        let addr = f.read_u32()?;
                        sev_secret_block = Some(SevSecretBlockGpa { addr, size });
                        entries_not_found_yet -= 1;
                    }
                    SEV_HASH_TABLE_RV_GUID => {
                        if entry_len != 0x1a || sev_hashes_table.is_some() {
                            return Err(ParseOvmfTableError::InvalidOvmfTableFormat);
                        }
                        let size = f.read_u32()?;
                        let addr = f.read_u32()?;
                        sev_hashes_table = Some(SevHashesTableGpa { addr, size });
                        entries_not_found_yet -= 1;
                    }
                    _ => {
                        f.offset -= entry_len - 16 - 2;
                    }
                }
            }

            if entries_not_found_yet != 0 {
                return Err(ParseOvmfTableError::InvalidOvmfTableFormat);
            }
            // Safety: safe because we just checked entries_not_found_yet.
            unsafe {
                Ok((
                    sev_secret_block.unwrap_unchecked(),
                    sev_hashes_table.unwrap_unchecked(),
                ))
            }
        }

        pub(crate) struct SevSecretBlockGpa {
            pub(crate) addr: u32,
            pub(crate) size: u32,
        }

        pub(crate) struct SevHashesTableGpa {
            pub(crate) addr: u32,
            pub(crate) size: u32,
        }
    }
}

/// TODO: add SEV-SNP related facilities.
pub mod snp {}

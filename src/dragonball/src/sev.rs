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
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SevStart {
        /// Whether the SEV VM will pause and wait for the injection of secrets
        /// after starting.
        pub paused: bool,
        /// The tenant's policy for this SEV guest.
        pub policy: sev::launch::sev::Policy,
        /// A struct containing the necessary certificate and session to
        /// establish a secure channel between AMD-SP and the tenant.
        pub secure_channel: Option<Box<SecureChannel>>,
    }

    /// A struct that contains the necessary certificate and session to
    /// establish a secure channel between AMD-SP and the tenant.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SecureChannel {
        /// The tenant's Diffie-Hellman certificate.
        pub cert: sev::certs::sev::sev::Certificate,
        /// A secure channel with the AMD SP.
        pub session: sev::launch::sev::Session,
    }

    pub struct SevSecretsInjection {
        pub secrets: Vec<sev::launch::sev::Secret>,
        pub resume_vm: bool,
    }
}

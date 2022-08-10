use std::fmt::{Debug, Display};

pub type CLIResult<T> = Result<T, CLIError>;


#[derive(Debug)]
pub enum CLIError {
    KernelError(KernelErrorKind),
    RootfsError(RootfsErrorKind),
    VmConfigError(String),
}

impl Display for CLIError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            CLIError::KernelError(ref err) => write!(f, "Error occurred about kernel: {:?}", err.as_str()),
            CLIError::RootfsError(ref err) => write!(f, "Error occurred about rootfs: {:?}", err.as_str()),
            CLIError::VmConfigError(ref msg) => write!(f, "Vm configuration related error: {:?}", msg),
        }
    }
}


#[derive(Debug, Clone, Copy)]
pub enum KernelErrorKind {
    KernelNotFound,
    KernelLoadFailed,
}

impl KernelErrorKind {
    fn as_str(&self) -> &str {
        match *self {
            KernelErrorKind::KernelNotFound => "Kernel not found.",
            KernelErrorKind::KernelLoadFailed => "Failed to load this kernel.",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RootfsErrorKind {
    RootfsNotFound,
    RootfsLoadFailed,
}

impl RootfsErrorKind {
    fn as_str(&self) -> &str {
        match *self {
            RootfsErrorKind::RootfsNotFound => "Rootfs not found",
            RootfsErrorKind::RootfsLoadFailed => "Failed to load the rootfs",
        }
    }
}

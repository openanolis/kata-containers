// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt;

#[macro_use]
extern crate slog;

pub mod device;
pub mod fs;
pub mod hooks;
pub mod k8s;
pub mod mount;
pub mod numa;
pub mod rand;
pub mod spec;
pub mod validate;

// Convenience macro to obtain the scoped logger
#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[macro_export]
macro_rules! eother {
    () => (std::io::Error::new(std::io::ErrorKind::Other, ""));
    ($fmt:expr, $($arg:tt)*) => ({
        std::io::Error::new(std::io::ErrorKind::Other, format!($fmt, $($arg)*))
    })
}


pub struct HexSlice<'a>(&'a [u8]);

impl<'a> HexSlice<'a> {
    pub fn new<T>(data: &'a T) -> HexSlice<'a>
    where
        T: ?Sized + AsRef<[u8]> + 'a,
    {
        HexSlice(data.as_ref())
    }
}

// You can even choose to implement multiple traits, like Lower and UpperHex
impl<'a> fmt::LowerHex for HexSlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0 {
            // Decide if you want to pad out the value here
            write!(f, "{:x}", byte)?;
        }
        Ok(())
    }
}
impl<'a> fmt::UpperHex for HexSlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{:X}", byte)?;
        }
        Ok(())
    }
}
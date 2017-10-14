//! GPIO interface
//!
//! The GPIO crate allows easy and fast access to GPIO pins. It aims to provide
//! an ergonomic interface while being as fast as possible, enabling
//! applications like bitbanging which require fast switching without
//! complicating simpler tasks.
//!
//! The core interface is defined using `GpioValue` and the `GpioOut` and
//! `GpioIn` traits. Concrete implementations are available in submodules.
//!
//! ### TODO
//!
//! * `GpioInput` trait
//! * `/dev/mem` interface

pub mod sysfs;

/// Value read from or written to a GPIO port.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GpioValue {
    Low,
    High,
}

impl From<bool> for GpioValue {
    fn from(val: bool) -> GpioValue {
        if val { GpioValue::High } else { GpioValue::Low }
    }
}

impl From<u8> for GpioValue {
    fn from(val: u8) -> GpioValue {
        if val != 0 {
            GpioValue::High
        } else {
            GpioValue::Low
        }
    }
}

/// Support sending `GPIOValue`s
pub trait GpioOut {
    type Error;

    #[inline(always)]
    fn set_value<T: Into<GpioValue> + Copy>(&mut self, value: T) -> Result<(), Self::Error> {
        match value.into() {
            GpioValue::High => self.set_high(),
            GpioValue::Low => self.set_low(),
        }
    }

    #[inline(always)]
    fn set_low(&mut self) -> Result<(), Self::Error>;

    #[inline(always)]
    fn set_high(&mut self) -> Result<(), Self::Error>;
}

//! Linux `/sys`-fs based GPIO control
//!
//! Uses filesystem operations to control GPIO ports. Very portable (across
//! devices running Linux), but incurs quite a bit of syscall overhead.

use std::{fs, io};
use std::io::Write;
use super::{GpioOut, GpioValue};

/// `/sys`-fs based GPIO output
#[derive(Debug)]
pub struct SysFsGpioOutput {
    gpio_num: u16,
    sysfp: fs::File,
    current_value: GpioValue,
}

impl SysFsGpioOutput {
    /// Open a GPIO port for Output.
    ///
    /// Will export the port if necessary.
    /// The port will be set to output mode. Note that the port will be
    /// unexported once the `SysFsGpioOutput` is dropped.
    ///
    /// A single file will be kept to avoid having to reopen the port every
    /// time the output changes.
    pub fn new(gpio_num: u16) -> io::Result<SysFsGpioOutput> {
        // export port first if not exported
        if let Err(_) = fs::metadata(&format!("/sys/class/gpio/gpio{}", gpio_num)) {
            let mut export_fp = fs::File::create("/sys/class/gpio/export")?;
            write!(export_fp, "{}", gpio_num)?;
        }

        // ensure we're using '0' as low
        fs::File::create(format!("/sys/class/gpio/gpio{}/active_low", gpio_num))?
            .write_all(b"0")?;

        // continue with initialization
        Self::exported_new(gpio_num)
    }

    /// Open an already exported GPIO port.
    /// Like `new`, but does not export the port.
    pub fn exported_new(gpio_num: u16) -> io::Result<SysFsGpioOutput> {
        /// set to output direction
        fs::File::create(format!("/sys/class/gpio/gpio{}/direction", gpio_num))?
            .write_all(b"out")?;

        // store open file handle
        let sysfp = fs::File::create(format!("/sys/class/gpio/gpio{}/value", gpio_num))?;

        Ok(SysFsGpioOutput {
               gpio_num: gpio_num,
               sysfp: sysfp,
               current_value: GpioValue::Low,
           })
    }
}

impl Drop for SysFsGpioOutput {
    fn drop(&mut self) {
        let unexport_fp = fs::File::create("/sys/class/gpio/unexport");

        if let Ok(mut fp) = unexport_fp {
            // best effort
            write!(fp, "{}\n", self.gpio_num).ok();
        }
    }
}

impl GpioOut for SysFsGpioOutput {
    type Error = io::Error;

    #[inline(always)]
    fn set_low(&mut self) -> io::Result<()> {
        if self.current_value == GpioValue::High {
            self.sysfp.write_all(b"0")?;
            self.current_value = GpioValue::Low;
        }
        Ok(())
    }

    #[inline(always)]
    fn set_high(&mut self) -> io::Result<()> {
        if self.current_value == GpioValue::Low {
            self.sysfp.write_all(b"1")?;
            self.current_value = GpioValue::High;
        }
        Ok(())
    }
}

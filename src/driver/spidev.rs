use crate::driver;
use nix::ioctl_write_buf;
use std::fs;
use std::os::unix::io::AsRawFd;
use std::path::Path;

ioctl_write_buf!(spi_ioc_wr_mode, b'k', 1, u8);
ioctl_write_buf!(spi_ioc_wr_lsb_first, b'k', 2, u8);
ioctl_write_buf!(spi_ioc_wr_max_speed_hz, b'k', 4, u32);

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum FirstBit {
    Lsb,
    Msb,
}

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub clock_polarity: u8,
    pub clock_phase: u8,
    pub first_bit: FirstBit,
    pub speed_hz: u32,
}

pub fn open(path: impl AsRef<Path>, conf: Config) -> Result<fs::File, driver::Error> {
    let spidev = fs::OpenOptions::new().write(true).open(path)?;
    let fd = spidev.as_raw_fd();

    let lsb_first: u8 = match conf.first_bit {
        FirstBit::Msb => 0,
        FirstBit::Lsb => 1,
    };
    unsafe {
        spi_ioc_wr_mode(fd, &[conf.clock_polarity | (conf.clock_polarity << 1)])?;
        spi_ioc_wr_lsb_first(fd, &[lsb_first])?;
        spi_ioc_wr_max_speed_hz(fd, &[conf.speed_hz])?;
    }

    Ok(spidev)
}

pub fn is_spidev(path: &Path) -> bool {
    let devs = regex::RegexSet::new(&[
        r"^/dev/spidev\d+\.\d+$",
        r"^/sys/devices/.+/spi\d\.\d$",
        r"^/sys/class/devices/.+/spi\d\.\d$",
    ])
    .unwrap();
    path.to_str().map(|s| devs.is_match(s)).unwrap_or(false)
}

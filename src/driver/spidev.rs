use std::fs;
use std::os::unix::io::AsRawFd;
use std::path;
use regex;
use device::*;
use driver;


ioctl!(write_ptr spi_ioc_wr_mode with b'k', 1; u8);
ioctl!(write_ptr spi_ioc_wr_lsb_first with b'k', 2; u8);
ioctl!(write_ptr spi_ioc_wr_max_speed_hz with b'k', 4; u32);

pub fn open(path: &path::PathBuf, dev: &Device, speed_hz: u32) -> Result<fs::File, driver::Error> {
    let spidev = fs::OpenOptions::new().write(true).open(path)?;
    let fd = spidev.as_raw_fd();

    let lsb_first: u8 = match dev.first_bit() {
        FirstBit::MSB => 0,
        FirstBit::LSB => 1,
    };
    unsafe {
        spi_ioc_wr_mode(fd, &(dev.clock_polarity() | (dev.clock_polarity() << 1)))?;
        spi_ioc_wr_lsb_first(fd, &lsb_first)?;
        spi_ioc_wr_max_speed_hz(fd, &speed_hz)?;
    }

    Ok(spidev)
}

pub fn is_spidev(path: &path::PathBuf) -> bool {
    let devs = regex::RegexSet::new(&[r"^/dev/spidev\d+\.\d+$",
                                      r"^/sys/devices/.+/spi\d\.\d$",
                                      r"^/sys/class/devices/.+/spi\d\.\d$"])
        .unwrap();
    devs.is_match(path.to_str().unwrap())
}

use device::*;
use std::fs;
use std::io;
use std::os::unix::io::AsRawFd;


ioctl!(write spi_ioc_wr_mode with b'k', 1; u8);
ioctl!(write spi_ioc_wr_lsb_first with b'k', 2; u8);
//ioctl!(write spi_ioc_wr_bits_per_word with b'k', 3; u8);
ioctl!(write spi_ioc_wr_max_speed_hz with b'k', 4; u32);
//ioctl!(write spi_ioc_wr_mode32 with b'k', 5; u32);

pub fn open<'f>(path: &'f str, dev: &Device) -> io::Result<Box<io::Write>> {
    let spidev = try!(fs::OpenOptions::new().write(true).open(path));
    let fd = spidev.as_raw_fd();

    let lsb_first: u8 = match dev.first_bit {
        FirstBit::MSB => 0,
        FirstBit::LSB => 1,
    };
    unsafe {
        spi_ioc_wr_mode(fd, &(dev.clock_polarity|(dev.clock_polarity<<1)));
        spi_ioc_wr_lsb_first(fd, &lsb_first);
        spi_ioc_wr_max_speed_hz(fd, &dev.speed_hz);
    }

    Ok(Box::new(spidev))
}

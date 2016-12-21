use std::io;
use std::thread;
use std::time;
use device::*;

pub struct Apa102 {
    /// 5-bit grayscale value to apply to all pixels.
    pub grayscale: u8,
}

impl Device for Apa102 {

    fn clock_phase(&self) -> u8 {
        0
    }

    fn clock_polarity(&self) -> u8 {
        0
    }

    fn first_bit(&self) -> FirstBit {
        FirstBit::MSB
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        try!(writer.write_all(&[0x00; 4]));
        for pix in pixels {
            try!(writer.write_all(&[0b11100000 | self.grayscale, pix.r, pix.g, pix.b]));
        }
        thread::sleep(time::Duration::new(0, 500_000));
        Ok(())
    }

}

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

    fn speed_hz(&self) -> u32 {
        500_000
    }

    fn write_pixel(&self, writer: &mut io::Write, pixel: &Pixel) -> io::Result<()> {
        writer.write_all(&[0b11100000 | self.grayscale, pixel.r, pixel.g, pixel.b])
    }

    fn begin_frame(&self, writer: &mut io::Write) -> io::Result<()> {
        writer.write_all(&[0x00; 4])
    }

    fn end_frame(&self, _: &mut io::Write) -> io::Result<()> {
        thread::sleep(time::Duration::new(0, 500_000));
        Ok(())
    }

}

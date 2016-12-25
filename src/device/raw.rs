use std::io;
use device::*;

pub struct Raw {
    clock_phase:    u8,
    clock_polarity: u8,
    first_bit:      FirstBit,
}

impl Default for Raw {

    fn default() -> Raw {
        Raw {
            clock_phase:    0,
            clock_polarity: 0,
            first_bit:      FirstBit::MSB
        }
    }

}

impl Device for Raw {

    fn clock_phase(&self) -> u8 {
        self.clock_phase
    }

    fn clock_polarity(&self) -> u8 {
        self.clock_polarity
    }

    fn first_bit(&self) -> FirstBit {
        self.first_bit.clone()
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        for pix in pixels.iter() {
            try!(writer.write_all(&[pix.g, pix.r, pix.b]));
        }
        Ok(())
    }

}


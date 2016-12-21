use std::io;

pub mod apa102;
pub mod lpd8806;

pub enum FirstBit { LSB, MSB }

pub trait Device {
    fn clock_phase(&self) -> u8;
    fn clock_polarity(&self) -> u8;
    fn first_bit(&self) -> FirstBit;
    fn write_frame(&self, &mut io::Write, &[Pixel]) -> io::Result<()>;
}

pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel {

    pub fn read_rgb24(reader: &mut io::Read) -> io::Result<Pixel> {
        let mut pixbuf: [u8; 3] = [0; 3];
        try!(reader.read_exact(&mut pixbuf));
        Ok(Pixel{ b: pixbuf[0], g: pixbuf[1], r: pixbuf[2] })
    }

}

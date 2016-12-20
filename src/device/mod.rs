use std::io;

pub enum FirstBit { LSB, MSB }

pub struct Device {
    pub clock_phase:    u8,
    pub clock_polarity: u8,
    pub first_bit:      FirstBit,
    pub speed_hz:       u32,

    pub write_pixel: fn(&mut io::Write, &Pixel) -> io::Result<()>,
    pub begin_frame: fn(&mut io::Write) -> io::Result<()>,
    pub end_frame:   fn(&mut io::Write) -> io::Result<()>,
}

pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel {

    pub fn read_rgb24(reader: &mut io::Read) -> io::Result<Pixel> {
        let mut pixbuf: [u8; 3] = [0; 3];
        match reader.read_exact(&mut pixbuf) {
            Ok(_)  => Ok(Pixel{ b: pixbuf[0], g: pixbuf[1], r: pixbuf[2] }),
            Err(e) => Err(e),
        }
    }

}

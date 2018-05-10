use std::*;
use std::io::Write;
use ::color::*;

const PORT: u16 = 5577;

pub struct Bulb {
    conn: net::TcpStream,
}

impl Bulb {
    pub fn connect(ip: net::IpAddr) -> io::Result<Bulb> {
        let conn = net::TcpStream::connect((ip, PORT)).unwrap();
        conn.set_read_timeout(Some(time::Duration::from_millis(100)))?;
        Ok(Bulb{ conn })
    }

    pub fn set_constant_color(&mut self, pix: &Pixel) -> io::Result<()> {
        // For proto:
        // -> [0x81, 0x8a, 0x8b, 0x96]
        // <- [129, 51, 35, 97, 1, 1, 0, 0, 0, 0, 4, 0, 0, 62]
        // -> [0x10, 0x14, 0x12, 0x05, 0x06, 0x15, 0x03, 0x0f0, 0x0a, 0x00, 0x0f, b'~']
        // <- []
        self.send_with_checksum(&[0x31, pix.r, pix.g, pix.b, 0x00, 0x00, 0x0f])
    }

    fn send_with_checksum(&mut self, data: &[u8]) -> io::Result<()> {
        let checksum = data.iter()
            .fold(0, |accum, b| accum + u32::from(*b)) as u8;
        let buf: Vec<u8> = data.iter()
            .cloned()
            .chain(iter::once(checksum))
            .collect();
        self.conn.write_all(&buf)
    }
}

pub struct Display {
    pub bulbs: Vec<Bulb>,
    pub buf: Vec<u8>,
}

impl io::Write for Display {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let need = self.bulbs.len() * 3;
        let n = self.buf.write(buf)?;
        if self.buf.len() >= need {
            self.flush()?;
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        for (bulb, chunk) in self.bulbs.iter_mut().zip(self.buf.chunks(3)) {
            bulb.set_constant_color(&Pixel{ r: chunk[0], g: chunk[1], b: chunk[2] })?;
        }
        self.buf.clear();
        Ok(())
    }
}

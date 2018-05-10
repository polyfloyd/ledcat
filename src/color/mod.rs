use std::io;


#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel {
    pub fn read_rgb24<R: io::Read>(mut reader: R) -> io::Result<Pixel> {
        let mut pixbuf: [u8; 3] = [0; 3];
        reader.read_exact(&mut pixbuf)?;
        Ok(Pixel {
            r: pixbuf[0],
            g: pixbuf[1],
            b: pixbuf[2],
        })
    }
}


pub struct Correction {
    r: Vec<u8>,
    g: Vec<u8>,
    b: Vec<u8>,
}

impl Correction {
    pub fn none() -> Correction {
        Correction {
            r: (0..=255).collect(),
            g: (0..=255).collect(),
            b: (0..=255).collect(),
        }
    }

    // https://en.wikipedia.org/wiki/SRGB
    pub fn srgb(max_red: u8, max_green: u8, max_blue: u8) -> Correction {
        let srgb = |x: f64| -> f64 {
            if x <= 0.04045 {
                return x / 12.92;
            }
            f64::powf((x + 0.055) / (1.0 + 0.055), 2.4)
        };
        let comp = |max| {
            (0..256)
                .map(|i| f64::round(srgb(f64::from(i) / 255.0) * f64::from(max)) as u8)
                .collect()
        };
        Correction {
            r: comp(max_red),
            g: comp(max_green),
            b: comp(max_blue),
        }
    }

    pub fn correct(&self, pix: &Pixel) -> Pixel {
        Pixel {
            r: self.r[pix.r as usize],
            g: self.g[pix.g as usize],
            b: self.b[pix.b as usize],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::*;
    use super::*;

    #[test]
    fn pixel_read_rgb24() {
        let mut c = io::Cursor::new([1, 2, 3, 4, 5, 6]);
        assert_eq!(Pixel { r: 1, g: 2, b: 3 },
                   Pixel::read_rgb24(&mut c).unwrap());
        assert_eq!(Pixel { r: 4, g: 5, b: 6 },
                   Pixel::read_rgb24(&mut c).unwrap());
    }
}

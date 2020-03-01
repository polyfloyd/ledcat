#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub struct Correction {
    r: [u8; 256],
    g: [u8; 256],
    b: [u8; 256],
}

fn collect_u8(bytes: impl Iterator<Item = u8>) -> [u8; 256] {
    let mut buf = [0; 256];
    for (i, b) in bytes.enumerate() {
        buf[i] = b;
    }
    buf
}

impl Correction {
    pub fn none() -> Correction {
        Correction {
            r: collect_u8(0..=255),
            g: collect_u8(0..=255),
            b: collect_u8(0..=255),
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
            let iter =
                (0..256).map(|i| f64::round(srgb(f64::from(i) / 255.0) * f64::from(max)) as u8);
            collect_u8(iter)
        };
        Correction {
            r: comp(max_red),
            g: comp(max_green),
            b: comp(max_blue),
        }
    }

    pub fn correct(&self, pix: Pixel) -> Pixel {
        Pixel {
            r: self.r[pix.r as usize],
            g: self.g[pix.g as usize],
            b: self.b[pix.b as usize],
        }
    }
}

use std::io;
use std::thread;
use std::time;
use clap;
use color::*;
use device::*;


pub struct Ws2812 { }

impl Device for Ws2812 {
    fn color_correction(&self) -> Correction {
        Correction::srgb(255, 255, 255)
    }

    fn spidev_config(&self) -> Option<spidev::Config> {
        Some(spidev::Config {
            clock_polarity: 0, // N/A: The WS2812 does require a clock.
            clock_phase: 0, // N/A
            first_bit: spidev::FirstBit::MSB,
            // 1.25 are required to transmit a single bit to the WS2812. The value of a bit is
            // determined by two periods, a high one and a low one. When the period is 1/3 high,
            // the bit is 0, when the period is 2/3rds high, the bit is 1. A period is transmitted
            // as 3 SPI bits, thus a single bit = 1s / 1.25µs * 3 = 2.4MHz
            speed_hz: 2_400_000,
        })
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        let buf: Vec<u8> = pixels.iter()
            .flat_map(|pix| vec![ pix.g, pix.r, pix.b ])
            .flat_map(|b| {
                let mut obits: u32 = 0;
                for i in 0..8 {
                    if (b >> i) & 1 == 1 {
                        obits |= 0b110 << (i * 3);
                    } else {
                        obits |= 0b100 << (i * 3);
                    }
                }
                vec![
                    ((obits >> 16) & 0xff) as u8,
                    ((obits >> 8) & 0xff) as u8,
                    (obits & 0xff) as u8,
                ]
            })
            .collect();
        writer.write_all(&buf)?;
        thread::sleep(time::Duration::new(0, 80_000)); // Sleep for 50µs to reset
        Ok(())
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("ws2812")
}

pub fn from_command(_: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    Ok(FromCommand::Device(Box::new(Ws2812 { })))
}

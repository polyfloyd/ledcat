use crate::device::*;
use clap;
use std::io;

pub enum Format {
    RGB24,
    RGB16,
    RGB12,
    RGB8,
    GS1,
}

pub struct Generic {
    pub format: Format,
}

impl Device for Generic {
    fn color_correction(&self) -> Correction {
        Correction::none()
    }

    fn write_frame(&self, writer: &mut dyn io::Write, pixels: &[Pixel]) -> io::Result<()> {
        match self.format {
            Format::RGB24 => {
                let buf: Vec<u8> = pixels
                    .iter()
                    .flat_map(|pix| vec![pix.r, pix.g, pix.b])
                    .collect();
                writer.write_all(&buf)?;
            }
            Format::RGB16 => {
                let buf: Vec<u8> = pixels
                    .iter()
                    .flat_map(|pix| {
                        vec![
                            (pix.r & 0xf8) | (pix.g >> 5),
                            (pix.g & 0x08) << 5 | (pix.b >> 3),
                        ]
                    })
                    .collect();
                writer.write_all(&buf)?;
            }
            Format::RGB12 => {
                let buf: Vec<u8> = pixels
                    .chunks(2)
                    .flat_map(|ch| {
                        let (a, b) = (&ch[0], ch.get(1).cloned().unwrap_or_default());
                        vec![
                            (a.r & 0xf0) | (a.g >> 4),
                            (a.b & 0xf0) | (b.r >> 4),
                            (b.g & 0xf0) | (b.b >> 4),
                        ]
                    })
                    .collect();
                writer.write_all(&buf)?;
            }
            Format::RGB8 => {
                let buf: Vec<u8> = pixels
                    .iter()
                    .map(|p| (p.b & 0xc0) | ((p.g >> 2) & 0x3c) | (p.r & 0x3))
                    .collect();
                writer.write_all(&buf)?;
            }
            Format::GS1 => {
                assert!(pixels.len() % 8 == 0);
                let prebuf: Vec<u8> = pixels
                    .iter()
                    .map(|p| if grayscale(*p) > 127 { 1 } else { 0 })
                    .collect();
                let packed: Vec<u8> = prebuf
                    .chunks(8)
                    .map(|chunk| {
                        chunk
                            .iter()
                            .enumerate()
                            .fold(0, |pack, (i, b)| pack | b << i)
                    })
                    .collect();
                writer.write_all(&packed)?;
            }
        }
        Ok(())
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("generic")
        .about("Output data as RGB24 or another pixel format")
        .arg(
            clap::Arg::with_name("format")
                .short("f")
                .long("format")
                .takes_value(true)
                .default_value("rgb24")
                .possible_values(&["rgb24", "rgb16", "rgb12", "rgb8", "gs1"]),
        )
}

pub fn from_command(args: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    let format = match args.value_of("format").unwrap() {
        "rgb16" => Format::RGB16,
        "rgb12" => Format::RGB12,
        "rgb8" => Format::RGB8,
        "gs1" => Format::GS1,
        _ => Format::RGB24,
    };
    Ok(FromCommand::Device(Box::new(Generic { format })))
}

fn grayscale(p: Pixel) -> u8 {
    let g = (0.2125 * p.r as f32) + (0.7154 * p.g as f32) + (0.0721 * p.b as f32);
    g.round() as u8
}

use std::io;
use device::*;
use clap;


pub enum Format {
    RGB24,
    RGB16,
    RGB12,
}

pub struct Generic {
    pub format: Format
}

impl Device for Generic {
    fn color_correction(&self) -> Correction {
        Correction::none()
    }

    fn write_frame(&self, writer: &mut io::Write, pixels: &[Pixel]) -> io::Result<()> {
        match self.format {
            Format::RGB24 => {
                let buf: Vec<u8> = pixels.iter()
                    .flat_map(|pix| vec![pix.r, pix.g, pix.b])
                    .collect();
                writer.write_all(&buf)?;
            },
            Format::RGB16 => {
                let buf: Vec<u8> = pixels.iter()
                    .flat_map(|pix| {
                        vec![
                            (pix.r & 0xf8) | (pix.g >> 5),
                            (pix.g & 0x08) << 5 | (pix.b >> 3),
                        ]
                    })
                    .collect();
                writer.write_all(&buf)?;
            },
            Format::RGB12 => {
                let buf: Vec<u8> = pixels.chunks(2)
                    .flat_map(|ch| {
                        let (a, b) = (&ch[0], ch.get(1).map(|p| *p).unwrap_or_default());
                        vec![
                            (a.r & 0xf0) | (a.g >> 4),
                            (a.b & 0xf0) | (b.r >> 4),
                            (b.g & 0xf0) | (b.b >> 4),
                        ]
                    })
                    .collect();
                writer.write_all(&buf)?;
            },
        }
        Ok(())
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("generic")
        .about("Output data as RGB24 or another pixel format")
        .arg(clap::Arg::with_name("format")
            .short("f")
            .long("format")
            .takes_value(true)
            .default_value("rgb24")
            .possible_values(&["rgb24", "rgb16", "rgb12"]))
}

pub fn from_command(args: &clap::ArgMatches, _: &GlobalArgs) -> io::Result<FromCommand> {
    let format = match args.value_of("format").unwrap() {
        "rgb16" => Format::RGB16,
        "rgb12" => Format::RGB12,
        _ => Format::RGB24,
    };
    Ok(FromCommand::Device(Box::new(Generic { format })))
}

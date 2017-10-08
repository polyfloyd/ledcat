#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]
#![cfg_attr(feature="clippy", allow(type_complexity))]

extern crate byteorder;
extern crate clap;
#[macro_use]
extern crate derive_error;
extern crate gpio;
extern crate libc;
#[macro_use]
extern crate nix;
extern crate regex;

use std::borrow::Borrow;
use std::collections;
use std::env;
use std::fs;
use std::io;
use std::ops::DerefMut;
use std::path;
use std::thread;
use std::time;
use ::color::*;
use ::device::*;
use ::driver::*;
use ::input::*;
use ::input::geometry::*;

#[macro_use]
mod util;
mod color;
mod device;
mod driver;
mod input;


fn main() {
    let geom_default = match env::var("LEDCAT_GEOMETRY") {
        Ok(s) => s,
        Err(_) => "".to_string(),
    };
    let mut cli = clap::App::new("ledcat")
        .version("0.0.1")
        .author("polyfloyd <floyd@polyfloyd.net>")
        .about("Like netcat, but for leds.")
        .arg(clap::Arg::with_name("output")
            .short("o")
            .long("output")
            .takes_value(true)
            .default_value("-")
            .help("The output file to write to. Use - for stdout."))
        .arg(clap::Arg::with_name("input")
            .short("i")
            .long("input")
            .takes_value(true)
            .min_values(1)
            .multiple(true)
            .default_value("-")
            .help("The inputs to read from. Read the manual for how inputs are read and \
                   prioritized."))
        .arg(clap::Arg::with_name("linger")
            .short("l")
            .long("linger")
            .help("Keep trying to read from the input(s) after EOF is reached"))
        .arg(clap::Arg::with_name("clear-timeout")
            .long("clear-timeout")
            .takes_value(true)
            .default_value("100")
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .conflicts_with("framerate")
            .help("Sets a timeout in milliseconds after which partially read frames are deleted.\
                   If a framerate is set, a timeout is calculated automatically."))
        .arg(clap::Arg::with_name("geometry")
            .short("g")
            .long("geometry")
            .alias("num-pixels")
            .takes_value(true)
            .default_value(&geom_default)
            .validator(|val| match val.parse::<Dimensions>() {
                Ok(_) => Ok(()),
                Err(err) => Err(format!("{}", err)),
            })
            .help("Specify the size of a two dimensional display"))
        .arg(clap::Arg::with_name("transpose")
            .short("t")
            .long("transpose")
            .takes_value(true)
            .min_values(1)
            .multiple(true)
            .possible_values(&["reverse", "zigzag_x", "zigzag_y", "mirror_x", "mirror_y"])
            .help("Apply one or more transpositions to the output"))
        .arg(clap::Arg::with_name("color-correction")
            .short("c")
            .long("color-correction")
            .takes_value(true)
            .possible_values(&["none", "srgb"])
            .help("Override the default color correction. The default is determined per device."))
        .arg(clap::Arg::with_name("dim")
            .long("dim")
            .takes_value(true)
            .default_value("1.0")
            .validator(|v| {
                let f = v.parse::<f32>()
                    .map_err(|e| format!("{}", e))?;
                if 0.0 <= f && f <= 1.0 {
                    Ok(())
                } else {
                    Err(format!("dim value out of range: {}", f))
                }
            })
            .help("Apply a global grayscale before the collor correction. The value should be \
                   between 0 and 1.0 inclusive"))
        .arg(clap::Arg::with_name("driver")
            .long("driver")
            .takes_value(true)
            .help("The driver to use for the output. If this is not specified, the driver is \
                   automaticaly detected based on the output"))
        .arg(clap::Arg::with_name("serial-baudrate")
            .long("serial-baudrate")
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .default_value("1152000")
            .help("If serial is used as driver, use this to set the baudrate"))
        .arg(clap::Arg::with_name("framerate")
            .short("f")
            .long("framerate")
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .help("Limit the number of frames per second"))
        .arg(clap::Arg::with_name("single-frame")
            .short("1")
            .long("one")
            .conflicts_with("framerate")
            .help("Send a single frame to the output and exit"))
        .subcommand(artnet::command())
        .subcommand(hub75::command());

    let mut device_constructors = collections::HashMap::new();
    for device_init in device::devices() {
        device_constructors.insert(device_init.0.get_name().to_string(), device_init.1);
        cli = cli.subcommand(device_init.0);
    }

    let matches = cli.clone().get_matches();
    let (sub_name, sub_matches) = matches.subcommand();
    if sub_name == "" {
        let mut out = io::stderr();
        cli.write_help(&mut out).unwrap();
        eprintln!();
        return;
    }

    // Don't require the display geomtry to be set just yet, a non-outputting subcommand may not
    // need it anyway.
    let maybe_dimensions = matches.value_of("geometry")
        .and_then(|v| v.parse().ok());

    let mut output: Box<Output> = if sub_name == "artnet" {
        match artnet::from_command(sub_matches.unwrap(), maybe_dimensions).unwrap() {
            Some(output) => output,
            None => return, // If a non-outputting subcommand has been executed.
        }

    } else if sub_name == "hub75" {
        let (w, h) = match maybe_dimensions {
            None|Some(geometry::Dimensions::One(_)) => {
                eprintln!("hub75 requires 2D geometry");
                return;
            },
            Some(geometry::Dimensions::Two(w, h)) => (w, h),
        };
        Box::new(hub75::from_command(sub_matches.unwrap(), w, h).unwrap())

    } else {
        let dev = device_constructors[sub_name](sub_matches.unwrap());
        let output_file = path::PathBuf::from(match matches.value_of("output").unwrap() {
            "-" => "/dev/stdout",
            _ => matches.value_of("output").unwrap(),
        });

        let driver_name = matches.value_of("driver")
            .map(|s: &str| s.to_string())
            .or_else(|| driver::detect(&output_file));
        let driver_name = match driver_name {
            Some(n) => n,
            None => {
                eprintln!("Unable to determine the driver to use. Please set one using --driver.");
                return;
            }
        };
        let output: Box<io::Write> = match driver_name.as_str() {
            "none" => Box::new(fs::OpenOptions::new().write(true).open(&output_file).unwrap()),
            "spidev" => {
                Box::new(spidev::open(&output_file, dev.borrow()).unwrap())
            },
            "serial" => {
                let baudrate = matches.value_of("serial-baudrate").unwrap().parse::<u32>().unwrap();
                Box::new(serial::open(&output_file, baudrate).unwrap())
            },
            _ => {
                eprintln!("Unknown driver {}", driver_name);
                return;
            }
        };
        Box::new((dev, output))
    };
    let dimensions = match maybe_dimensions {
        Some(d) => d,
        None => {
            eprintln!("Please set the frame size through either --num-pixels or --geometry");
            return;
        },
    };

    let transpose = matches.values_of("transpose")
        .map(|v| v.collect())
        .unwrap_or_else(Vec::new);
    let transposition = match transposition_table(&dimensions, transpose) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("{}", err);
            return;
        }
    };
    assert_eq!(dimensions.size(), transposition.len());

    let color_correction = matches.value_of("color-correction")
        .and_then(|name| match name {
            "none" => Some(Correction::none()),
            "srgb" => Some(Correction::srgb(255, 255, 255)),
            _ => None,
        })
        .unwrap_or_else(|| output.color_correction());
    let dim = (matches.value_of("dim")
            .unwrap()
            .parse::<f32>()
            .unwrap() * 255.0)
        .round() as u8;

    let frame_interval = matches.value_of("framerate")
        .map(|fps| time::Duration::new(1, 0) / fps.parse::<u32>().unwrap());
    let single_frame = matches.is_present("single-frame");

    let inputs = matches.values_of("input").unwrap();
    let input_eof = if matches.is_present("linger") {
        select::WhenEOF::Retry
    } else {
        select::WhenEOF::Close
    };
    let files = inputs.map(|f| match f {
            "-" => "/dev/stdin",
            f => f,
        })
        .collect();
    let clear_timeout = frame_interval.map(|t| t * 2)
        .or_else(|| {
            matches.value_of("clear-timeout")
                .map(|v| v.parse::<u32>().unwrap())
                .map(|ms| time::Duration::new(0, ms * 1_000_000))
        });
    let mut input = select::Reader::from_files(files, dimensions.size() * 3, input_eof, clear_timeout).unwrap();

    loop {
        let start = time::Instant::now();
        if pipe_frame(&mut input, output.deref_mut(), &transposition, &color_correction, dim).is_err() {
            break;
        }
        if single_frame {
            break;
        }
        if let Some(interval) = frame_interval {
            let el = start.elapsed();
            if interval >= el {
                thread::sleep(interval - el);
            }
        }
    }
}

fn pipe_frame<R>(mut input: R,
                 dev: &mut Output,
                 transposition: &[usize],
                 correction: &Correction,
                 dim: u8)
                 -> io::Result<()>
                 where R: io::Read {
    // Read a full frame into a buffer. This prevents half frames being written to a
    // potentially timing sensitive output if the input blocks and lets us apply the
    // transpositions.
    let mut buffer = vec![Pixel { r: 0, g: 0, b: 0 }; transposition.len()];
    for transpose_mapped in transposition {
        let pix_in = Pixel::read_rgb24(&mut input)?;
        let pix_dimmed = {
            let dim16 = u16::from(dim);
            Pixel {
                r: ((u16::from(pix_in.r) * dim16) / 0xff) as u8,
                g: ((u16::from(pix_in.g) * dim16) / 0xff) as u8,
                b: ((u16::from(pix_in.b) * dim16) / 0xff) as u8,
            }
        };
        let pix_corrected = correction.correct(pix_dimmed);
        buffer[*transpose_mapped] = pix_corrected;
    }
    dev.output_frame(&buffer)?;
    Ok(())
}

fn transposition_table(dimensions: &Dimensions,
                       operations: Vec<&str>)
                       -> Result<Vec<usize>, String> {
    let transpositions: Vec<Box<Transposition>> = try!(operations.into_iter()
        .map(|name| -> Result<Box<Transposition>, String> {
            match (name, *dimensions) {
                ("reverse", dim) => Ok(Box::from(Reverse { length: dim.size() })),
                ("zigzag_x", Dimensions::Two(w, h)) | ("zigzag_y", Dimensions::Two(w, h)) => {
                    Ok(Box::from(Zigzag {
                        width: w,
                        height: h,
                        major_axis: match name.chars().last().unwrap() {
                            'x' => Axis::X,
                            'y' => Axis::Y,
                            _ => unreachable!(),
                        },
                    }))
                },
                ("mirror_x", Dimensions::Two(w, h)) | ("mirror_y", Dimensions::Two(w, h)) => {
                    Ok(Box::from(Mirror {
                        width: w,
                        height: h,
                        axis: match name.chars().last().unwrap() {
                            'x' => Axis::X,
                            'y' => Axis::Y,
                            _ => unreachable!(),
                        },
                    }))
                },
                (name, Dimensions::One(_)) => Err(format!("{} requires 2D geometry to be specified", name)),
                (name, _) => Err(format!("Unknown transposition: {}", name)),
            }
        })
        .collect());
    Ok((0..dimensions.size())
        .map(|index| transpositions.transpose(index))
        .collect())
}

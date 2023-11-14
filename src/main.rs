#[macro_use]
mod util;
mod color;
mod device;
mod driver;
mod input;

use crate::color::*;
use crate::device::*;
use crate::driver::*;
use crate::input::geometry::*;
use crate::input::*;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::iter;
use std::path::PathBuf;
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let mut cli = clap::command!()
        .arg(clap::arg!(-o --output <file> "The output file to write to. Use - for stdout.")
            .default_value("-"))
        .arg(clap::arg!(-i --input <file> ... "The inputs to read from. Read the manual for how inputs are read and prioritized.")
            .default_value("-"))
        .arg(clap::arg!(-e --exit <value> "Set the exit condition. \"one\" and \"all\" indicate the number of files that should be closed to trigger")
            .value_parser(["never", "one", "all"])
            .default_value("all"))
        .arg(clap::arg!(--"clear-timeout" <value> "Sets a timeout in milliseconds after which partially read frames are deleted. If a framerate is set, a timeout is calculated automatically.")
            .value_parser(clap::value_parser!(u32))
            .conflicts_with("framerate"))
        .arg(clap::arg!(-g --geometry <value> "Specify the size of the display. Can be either a number for 1D, WxH for 2D, or \"env\" to load the LEDCAT_GEOMETRY environment variable.")
            .alias("num-pixels")
            .default_value("env")
            .value_parser(|val: &str| {
                if val == "env" {
                    return Ok(())
                }
                match val.parse::<Dimensions>() {
                    Ok(_) => Ok(()),
                    Err(err) => Err(err),
                }
            }))
        .arg(clap::arg!(-t --transpose <value> ... "Apply one or more transpositions to the output")
            .value_parser(["reverse", "zigzag_x", "zigzag_y", "mirror_x", "mirror_y"]))
        .arg(clap::arg!(-c --"color-correction" <value> "Override the default color correction. The default is determined per device.")
            .value_parser(["none", "srgb"]))
        .arg(clap::arg!(--dim <value> "Apply a global grayscale before the collor correction. The value should be between 0 and 1.0 inclusive")
            .default_value("1.0")
            .value_parser(clap::value_parser!(f32)))
        .arg(clap::arg!(--driver <value> "The driver to use for the output. If this is not specified, the driver is automaticaly detected based on the output"))
        .arg(clap::arg!(--"serial-baudrate" <value>  "If serial is used as driver, use this to set the baudrate")
            .value_parser(clap::value_parser!(u32))
            .default_value("1152000"))
        .arg(clap::arg!(-f --framerate <value> "Limit the number of frames per second")
            .value_parser(clap::value_parser!(u32)))
        .arg(clap::arg!(-'1' --one "Send a single frame to the output and exit")
            .conflicts_with("framerate"));

    let mut device_constructors = BTreeMap::new();
    for (command, from_command) in device::devices() {
        device_constructors.insert(command.get_name().to_string(), from_command);
        cli = cli.subcommand(command);
    }

    let matches = cli.clone().get_matches();
    let (sub_name, sub_matches) = match matches.subcommand() {
        Some(v) => v,
        None => {
            let mut out = io::stderr();
            cli.write_help(&mut out).unwrap();
            eprintln!();
            process::exit(1);
        }
    };

    let gargs = GlobalArgs {
        output_file: {
            let output = matches.get_one::<String>("output").unwrap();
            PathBuf::from(match output.as_str() {
                "-" => "/dev/stdout",
                out => out,
            })
        },
        // Don't require the display geomtry to be set just yet, a non-outputting subcommand may
        // not need it anyway.
        dimensions: {
            let env = env::var("LEDCAT_GEOMETRY");
            match matches.get_one::<String>("geometry").unwrap().as_str() {
                "env" => match env.as_ref().map(|e| e.as_str()) {
                    Err(_) | Ok("") => None,
                    Ok(e) => Some(e),
                },
                v => Some(v),
            }
            .and_then(|v| v.parse().ok())
        },
    };
    let output: Box<dyn Output> = {
        let from_command = device_constructors[sub_name](sub_matches, &gargs)?;
        match from_command {
            FromCommand::Device(dev) => {
                let driver_name = matches
                    .get_one::<String>("driver")
                    .map(|s| s.as_str())
                    .or_else(|| driver::detect(&gargs.output_file))
                    .map(str::to_string)
                    .unwrap_or_else(|| "none".to_string());
                let output: Box<dyn io::Write + Send> = match driver_name.as_str() {
                    "none" => Box::new(
                        fs::OpenOptions::new()
                            .write(true)
                            .open(&gargs.output_file)
                            .unwrap(),
                    ),
                    "serial" => {
                        let baudrate = matches.get_one::<u32>("serial-baudrate").unwrap();
                        Box::new(serial::open(&gargs.output_file, *baudrate).unwrap())
                    }
                    d => return Err(GenericError::new(format!("unknown driver {}", d)).into()),
                };
                Box::new((dev, output))
            }
            FromCommand::Output(output) => output,
            FromCommand::SubcommandHandled => return Ok(()),
        }
    };
    let dimensions = gargs.dimensions()?;

    let transposition = match matches.get_many::<String>("transpose") {
        Some(v) => transposition_table(&dimensions, v.map(|s| s.as_str())),
        None => transposition_table(&dimensions, iter::empty()),
    }?;
    assert_eq!(dimensions.size(), transposition.len());

    let color_correction = matches
        .get_one::<String>("color-correction")
        .map(String::as_str)
        .and_then(|name| match name {
            "none" => Some(Correction::none()),
            "srgb" => Some(Correction::srgb(255, 255, 255)),
            _ => None,
        })
        .unwrap_or_else(|| output.color_correction());
    let dim = (matches.get_one::<f32>("dim").unwrap().clamp(0.0, 1.0) * 255.0).round() as u8;

    let frame_interval = matches
        .get_one::<u32>("framerate")
        .map(|fps| Duration::from_secs(1) / *fps);
    let single_frame = matches.get_flag("one");

    let input = {
        let exit_condition = {
            match matches.get_one::<String>("exit").map(String::as_str) {
                Some("never") => select::ExitCondition::Never,
                Some("one") => select::ExitCondition::OneClosed,
                Some("all") | None => select::ExitCondition::AllClosed,
                Some(_) => unreachable!(),
            }
        };
        let files = matches
            .get_many::<String>("input")
            .unwrap()
            .map(|f| match f.as_str() {
                "-" => "/dev/stdin",
                f => f,
            })
            .collect();
        let clear_timeout = frame_interval.map(|t| t * 2).unwrap_or_else(|| {
            let ms = matches
                .get_one::<u32>("clear-timeout")
                .copied()
                .unwrap_or(100);
            Duration::from_millis(ms as u64)
        });
        select::Reader::from_files(
            files,
            dimensions.size() * 3,
            exit_condition,
            Some(clear_timeout),
        )
        .unwrap()
    };

    let _ = pipe_frames(
        input,
        output,
        transposition,
        color_correction,
        dim,
        single_frame,
        frame_interval,
    );
    Ok(())
}

fn pipe_frames(
    mut input: impl io::Read + Send + 'static,
    mut dev: impl Output + 'static,
    transposition: Vec<usize>,
    correction: Correction,
    dim: u8,
    single_frame: bool,
    frame_interval: Option<Duration>,
) -> io::Result<()> {
    let (err_tx, err_rx) = mpsc::channel();
    macro_rules! try_or_send {
        ($tx:expr, $expression:expr) => {
            match $expression {
                Ok(val) => val,
                Err(err) => {
                    $tx.send(Err(err)).unwrap();
                    return;
                }
            }
        };
    }

    let local_err_tx = err_tx.clone();
    let num_pixels = transposition.len();
    let (input_tx, input_rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        loop {
            // Read a full frame into a buffer. This prevents half frames being written to a
            // potentially timing sensitive output if the input blocks and lets us apply the
            // transpositions.
            let mut bin_buffer = vec![0; num_pixels * 3];
            try_or_send!(local_err_tx, input.read_exact(&mut bin_buffer));
            input_tx.send(bin_buffer).unwrap();
            if single_frame {
                break;
            }
        }
    });

    let (map_tx, map_rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        for bin_buffer in input_rx.into_iter() {
            let mut buffer = vec![Pixel { r: 0, g: 0, b: 0 }; transposition.len()];
            for (transpose_mapped, bin) in transposition.iter().zip(bin_buffer.chunks(3)) {
                // Load the pixel.
                let pix = Pixel {
                    r: bin[0],
                    g: bin[1],
                    b: bin[2],
                };
                // Apply dimming.
                let pix = {
                    let dim16 = u16::from(dim);
                    Pixel {
                        r: ((u16::from(pix.r) * dim16) / 0xff) as u8,
                        g: ((u16::from(pix.g) * dim16) / 0xff) as u8,
                        b: ((u16::from(pix.b) * dim16) / 0xff) as u8,
                    }
                };
                // Apply color correction.
                let pix = correction.correct(pix);
                // Apply transposition and store the pixel in the output buffer.
                buffer[*transpose_mapped] = pix;
            }
            map_tx.send(buffer).unwrap();
        }
    });

    thread::spawn(move || loop {
        let start = Instant::now();

        let buffer = match map_rx.recv() {
            Ok(v) => v,
            Err(_) => break,
        };
        try_or_send!(err_tx, dev.output_frame(&buffer));

        if let Some(interval) = frame_interval {
            let el = start.elapsed();
            if interval >= el {
                thread::sleep(interval - el);
            }
        }
    });

    match err_rx.recv() {
        Ok(err) => err,
        Err(mpsc::RecvError) => Ok(()),
    }
}

fn transposition_table<'a>(
    dimensions: &Dimensions,
    operations: impl Iterator<Item = &'a str>,
) -> Result<Vec<usize>, String> {
    let transpositions = operations
        .map(|name| map_transposition(dimensions, name))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((0..dimensions.size())
        .map(|index| transpositions.transpose(index))
        .collect())
}

fn map_transposition(
    dimensions: &Dimensions,
    name: &str,
) -> Result<Box<dyn Transposition>, String> {
    match (name, *dimensions) {
        ("reverse", dim) => Ok(Box::new(Reverse { length: dim.size() })),
        ("zigzag_x", Dimensions::Two(w, h)) | ("zigzag_y", Dimensions::Two(w, h)) => {
            Ok(Box::new(Zigzag {
                width: w,
                height: h,
                major_axis: match name.chars().last().unwrap() {
                    'x' => Axis::X,
                    'y' => Axis::Y,
                    _ => unreachable!(),
                },
            }))
        }
        ("mirror_x", Dimensions::Two(w, h)) | ("mirror_y", Dimensions::Two(w, h)) => {
            Ok(Box::new(Mirror {
                width: w,
                height: h,
                axis: match name.chars().last().unwrap() {
                    'x' => Axis::X,
                    'y' => Axis::Y,
                    _ => unreachable!(),
                },
            }))
        }
        (name, Dimensions::One(_)) => Err(format!("{} requires 2D geometry to be specified", name)),
        (name, _) => Err(format!("unknown transposition: {}", name)),
    }
}

#[derive(Debug)]
struct GenericError {
    msg: String,
}

impl GenericError {
    fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }
}

impl fmt::Display for GenericError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for GenericError {}

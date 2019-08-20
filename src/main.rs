#[macro_use]
extern crate derive_error;

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
use std::borrow::Borrow;
use std::collections;
use std::env;
use std::fs;
use std::io;
use std::path;
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time;

fn main() {
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
            .help("[deprecated] Keep trying to read from the input(s) after EOF is reached"))
        .arg(clap::Arg::with_name("exit")
            .short("e")
            .long("exit")
            .takes_value(true)
            .possible_values(&["never", "one", "all"])
            .default_value("all")
            .help("Set the exit condition"))
        .arg(clap::Arg::with_name("clear-timeout")
            .long("clear-timeout")
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .conflicts_with("framerate")
            .help("Sets a timeout in milliseconds after which partially read frames are deleted. \
                   If a framerate is set, a timeout is calculated automatically."))
        .arg(clap::Arg::with_name("geometry")
            .short("g")
            .long("geometry")
            .alias("num-pixels")
            .takes_value(true)
            .default_value("env")
            .validator(|val| {
                if val == "env" {
                    return Ok(())
                }
                match val.parse::<Dimensions>() {
                    Ok(_) => Ok(()),
                    Err(err) => Err(err.to_string()),
                }
            })
            .help("Specify the size of the display. Can be either a number for 1D, WxH for 2D, or\
                  \"env\" to load the LEDCAT_GEOMETRY environment variable."))
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
            .help("Send a single frame to the output and exit"));

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
        process::exit(1);
    }

    let gargs = GlobalArgs {
        // Don't require the display geomtry to be set just yet, a non-outputting subcommand may
        // not need it anyway.
        dimensions: {
            let env = env::var("LEDCAT_GEOMETRY");
            match matches.value_of("geometry").unwrap() {
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
        let result = device_constructors[sub_name](sub_matches.unwrap(), &gargs);
        let from_command = match result {
            Ok(v) => v,
            Err(err) => {
                println!("{}", err);
                return;
            }
        };
        match from_command {
            FromCommand::Device(dev) => {
                let output_file = path::PathBuf::from(match matches.value_of("output").unwrap() {
                    "-" => "/dev/stdout",
                    _ => matches.value_of("output").unwrap(),
                });

                let driver_name = matches
                    .value_of("driver")
                    .map(|s: &str| s.to_string())
                    .or_else(|| driver::detect(&output_file))
                    .unwrap_or_else(|| "none".to_string());
                let output: Box<dyn io::Write + Send> = match driver_name.as_str() {
                    "none" => Box::new(
                        fs::OpenOptions::new()
                            .write(true)
                            .open(&output_file)
                            .unwrap(),
                    ),
                    "spidev" => Box::new(spidev::open(&output_file, dev.borrow()).unwrap()),
                    "serial" => {
                        let baudrate = matches
                            .value_of("serial-baudrate")
                            .unwrap()
                            .parse::<u32>()
                            .unwrap();
                        Box::new(serial::open(&output_file, baudrate).unwrap())
                    }
                    _ => {
                        eprintln!("Unknown driver {}", driver_name);
                        return;
                    }
                };
                Box::new((dev, output))
            }
            FromCommand::Output(output) => output,
            FromCommand::SubcommandHandled => return,
        }
    };
    let dimensions = match gargs.dimensions() {
        Ok(d) => d,
        Err(err) => {
            eprintln!("{}", err);
            return;
        }
    };

    let transpose = matches
        .values_of("transpose")
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

    let color_correction = matches
        .value_of("color-correction")
        .and_then(|name| match name {
            "none" => Some(Correction::none()),
            "srgb" => Some(Correction::srgb(255, 255, 255)),
            _ => None,
        })
        .unwrap_or_else(|| output.color_correction());
    let dim = (matches.value_of("dim").unwrap().parse::<f32>().unwrap() * 255.0).round() as u8;

    let frame_interval = matches
        .value_of("framerate")
        .map(|fps| time::Duration::new(1, 0) / fps.parse::<u32>().unwrap());
    let single_frame = matches.is_present("single-frame");

    let inputs = matches.values_of("input").unwrap();
    let exit_condition = {
        let e = {
            matches.value_of("exit").unwrap_or_else(|| {
                if matches.is_present("linger") {
                    "never"
                } else {
                    "all"
                }
            })
        };
        match e {
            "never" => select::ExitCondition::Never,
            "one" => select::ExitCondition::OneClosed,
            "all" => select::ExitCondition::All,
            _ => unreachable!(),
        }
    };
    let files = inputs
        .map(|f| match f {
            "-" => "/dev/stdin",
            f => f,
        })
        .collect();
    let clear_timeout = frame_interval.map(|t| t * 2).unwrap_or_else(|| {
        let ms = matches
            .value_of("clear-timeout")
            .map(|v| v.parse::<u32>().unwrap())
            .unwrap_or(100);
        time::Duration::new(0, ms * 1_000_000)
    });
    let input = select::Reader::from_files(
        files,
        dimensions.size() * 3,
        exit_condition,
        Some(clear_timeout),
    )
    .unwrap();

    let _ = pipe_frames(
        input,
        output,
        transposition,
        color_correction,
        dim,
        single_frame,
        frame_interval,
    );
}

fn pipe_frames(
    mut input: impl io::Read + Send + 'static,
    mut dev: impl Output + 'static,
    transposition: Vec<usize>,
    correction: Correction,
    dim: u8,
    single_frame: bool,
    frame_interval: Option<time::Duration>,
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
        let start = time::Instant::now();

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
        Err(_) => Ok(()),
    }
}

fn transposition_table(
    dimensions: &Dimensions,
    operations: Vec<&str>,
) -> Result<Vec<usize>, String> {
    let rs: Result<Vec<_>, _> = operations
        .into_iter()
        .map(|name| -> Result<Box<dyn Transposition>, String> {
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
                (name, Dimensions::One(_)) => {
                    Err(format!("{} requires 2D geometry to be specified", name))
                }
                (name, _) => Err(format!("Unknown transposition: {}", name)),
            }
        })
        .collect();
    let transpositions = rs?;
    Ok((0..dimensions.size())
        .map(|index| transpositions.transpose(index))
        .collect())
}

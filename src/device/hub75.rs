use std::io;
use std::sync::mpsc;
use std::thread;
use color::*;
use device::*;
use clap;
use gpio::GpioOut;
use gpio::sysfs::SysFsGpioOutput;


struct Worker {
    width: usize,
    height: usize,

    pwm_cycles: u8,
    frame_rx: mpsc::Receiver<Vec<Pixel>>,
    err_tx: mpsc::Sender<io::Error>,
    cur_frame: Vec<Pixel>,

    level_select: Vec<SysFsGpioOutput>,
    rgb: Vec<[SysFsGpioOutput; 3]>,
    clock: SysFsGpioOutput,
    output_enable: SysFsGpioOutput,
    latch: SysFsGpioOutput,
}

impl Worker {
    fn run(&mut self) {
        loop {
            match self.frame_rx.try_recv() {
                Ok(frame) => {
                    assert_eq!(self.width * self.height, frame.len());
                    self.cur_frame = frame;
                },
                Err(mpsc::TryRecvError::Empty) => (),
                Err(_) => break,
            };
            for i in 0..self.pwm_cycles {
                let a = 255 / (self.pwm_cycles + 1);
                let min_val = 255 - i * a - a;
                if let Err(err) = self.refresh_display(min_val) {
                    self.err_tx.send(err).unwrap();
                }
            }
        }
    }

    fn refresh_display(&mut self, min_val: u8) -> io::Result<()> {
        let num_level_select = self.level_select.len();
        let scan_height = 1 << self.level_select.len();
        let scan_interleaved = (0..scan_height)
            .map(|i| ((i << 1) | (i >> (num_level_select - 1))) & (scan_height - 1));
        for y in scan_interleaved {
            // Clock in data for one row (Rn, Gn, Bn for data)
            for x in 0..self.width {
                for (line, rgb) in self.rgb.iter_mut().enumerate() {
                    let pix = &self.cur_frame[(y + line * scan_height) * self.width + x];
                    rgb[0].set_value(pix.r >= min_val)?;
                    rgb[1].set_value(pix.g >= min_val)?;
                    rgb[2].set_value(pix.b >= min_val)?;
                }
                // CLK pulse
                self.clock.set_value(1)?;
                self.clock.set_value(0)?;
            }
            // OE high
            self.output_enable.set_value(1)?;
            // Select line address (A, B, C, D)
            for (i, ls) in self.level_select.iter_mut().enumerate() {
                ls.set_value((y >> i) as u8 & 1)?;
            }
            // LAT pulse
            self.latch.set_value(1)?;
            self.latch.set_value(0)?;
            // OE low
            self.output_enable.set_value(0)?;
        }
        Ok(())
    }
}

pub struct Hub75 {
    frame_tx: mpsc::SyncSender<Vec<Pixel>>,
    err_rx: mpsc::Receiver<io::Error>,
}

impl Output for Hub75 {
    fn color_correction(&self) -> Correction {
        Correction::srgb(255, 255, 255)
    }

    fn output_frame(&mut self, frame: &[Pixel]) -> io::Result<()> {
        match self.err_rx.try_recv() {
            Ok(io_err) => return Err(io_err),
            Err(mpsc::TryRecvError::Empty) => (),
            Err(err) => return io_err!(Err(err)),
        };
        io_err!(self.frame_tx.send(frame.to_vec()))?;
        Ok(())
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("hub75")
        .about("Drive HUB75 LED-panels using GPIO")
        .arg(clap::Arg::with_name("level-select")
            .long("level-select")
            .takes_value(true)
            .validator(regex_validator!(r"^(?:[1-9]\d*,?)+$"))
            .help("The GPIO-pins connected to the level select. These are typically labeled as A, B, C and D"))
        .arg(clap::Arg::with_name("clock")
            .long("clock")
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .help("The GPIO-pin connected to the clock. Typically labeled as CLK"))
        .arg(clap::Arg::with_name("latch")
            .long("latch")
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .help("The GPIO-pin connected to the latch. Typically labeled as LAT"))
        .arg(clap::Arg::with_name("output-enable")
            .long("output-enable")
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .help("The GPIO-pin connected to the output-enable. Typically labeled as OE"))
        .arg(clap::Arg::with_name("red")
            .long("red")
            .takes_value(true)
            .validator(regex_validator!(r"^(?:[1-9]\d*,?)+$"))
            .help("The GPIO-pins connected to the red data lines. Typically labeled as R1 and R2"))
        .arg(clap::Arg::with_name("green")
            .long("green")
            .takes_value(true)
            .validator(regex_validator!(r"^(?:[1-9]\d*,?)+$"))
            .help("The GPIO-pins connected to the green data lines. Typically labeled as G1 and G2"))
        .arg(clap::Arg::with_name("blue")
            .long("blue")
            .takes_value(true)
            .validator(regex_validator!(r"^(?:[1-9]\d*,?)+$"))
            .help("The GPIO-pins connected to the blue data lines. Typically labeled as B1 and B2"))
        .arg(clap::Arg::with_name("pwm")
            .long("pwm")
            .default_value("3")
            .takes_value(true)
            .validator(regex_validator!(r"^[1-9]\d*$"))
            .help("The number of grayscale refreshes per frame that should be performed"))
}

pub fn from_command(args: &clap::ArgMatches, width: usize, height: usize) -> io::Result<Hub75> {
    let pwm_cycles = args.value_of("pwm").unwrap()
        .parse().unwrap();
    let pins = |name: &str| -> io::Result<Vec<SysFsGpioOutput>> {
        args.value_of(name).unwrap()
            .split(',')
            .map(|s| s.parse().unwrap())
            .map(|num| SysFsGpioOutput::new(num))
            .collect()
    };
    let pin = |name: &str| -> io::Result<SysFsGpioOutput> {
        Ok(pins(name)?.pop().unwrap())
    };

    let (frame_tx, frame_rx) = mpsc::sync_channel(0);
    let (err_tx, err_rx) = mpsc::channel();

    let mut worker = Worker {
        width,
        height,
        pwm_cycles,
        frame_rx,
        cur_frame: vec![Pixel::default(); width * height],
        err_tx,
        level_select: {
            let p = pins("level-select")?;
            if height % (1 << p.len()) != 0 {
                return Err(io::Error::new(io::ErrorKind::Other, "The height must be a multiple of 2^len(level-select-pins)"));
            }
            p
        },
        rgb: {
            let r = pins("red")?;
            let g = pins("green")?;
            let b = pins("blue")?;
            if r.len() != g.len() || g.len() != b.len() {
                return Err(io::Error::new(io::ErrorKind::Other, "The number of red, green and blue pins must be all equal"));
            }
            r.into_iter().zip(g).zip(b)
                .map(|a| [(a.0).0, (a.0).1, a.1])
                .collect()
        },
        clock: pin("clock")?,
        latch: pin("latch")?,
        output_enable: pin("output-enable")?,
    };
    thread::spawn(move || {
        worker.run();
    });
    Ok(Hub75 { frame_tx, err_rx })
}

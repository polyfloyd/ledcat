use clap;
use device::*;
use std::ffi::CString;
use std::mem;
use std::ptr;

pub struct LedMatrix {
    led_matrix: *mut RGBLedMatrix,
    backbuffer: *mut LedCanvas,
    width: usize,
    height: usize,
}

unsafe impl Send for LedMatrix {}

impl Output for LedMatrix {
    fn color_correction(&self) -> Correction {
        Correction::none()
    }

    fn output_frame(&mut self, frame: &[Pixel]) -> io::Result<()> {
        assert!(frame.len() == self.width * self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let pix = &frame[y * self.width + x];
                unsafe {
                    led_canvas_set_pixel(self.backbuffer, x as i32, y as i32, pix.r, pix.g, pix.b);
                }
            }
        }
        unsafe {
            self.backbuffer = led_matrix_swap_on_vsync(self.led_matrix, self.backbuffer);
        }
        Ok(())
    }
}

impl Drop for LedMatrix {
    fn drop(&mut self) {
        unsafe {
            let canvas = led_matrix_get_canvas(self.led_matrix);
            led_canvas_clear(canvas);
            led_matrix_delete(self.led_matrix);
        }
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("rpi-led-matrix")
        .about("Hzeller's Raspberry Pi LED Matrix library")
        .after_help("For a detailed guide and caveats, please refer to https://github.com/hzeller/rpi-rgb-led-matrix for more information")
        .arg(clap::Arg::with_name("rows")
            .long("led-rows")
            .takes_value(true)
            .required_unless("parallel")
            .validator(regex_validator!(r"^\d+$"))
            .help("The number of rows supported by the display, e.g. 32 or 16. The combined height of the display is rows * parallel"))
        .arg(clap::Arg::with_name("cols")
            .long("led-cols")
            .takes_value(true)
            .required_unless("chain")
            .validator(regex_validator!(r"^\d+$"))
            .help("The number of columns per panel. The combined width of the display is cols * chain"))
        .arg(clap::Arg::with_name("chain")
            .long("led-chain")
            .takes_value(true)
            .required_unless("cols")
            .validator(regex_validator!(r"^\d+$"))
            .help("The number of panels daisy chained together"))
        .arg(clap::Arg::with_name("parallel")
            .long("led-parallel")
            .takes_value(true)
            .required_unless("rows")
            .default_value("1")
            .validator(regex_validator!(r"^\d+$"))
            .help("The number of displays that are being driven in parallel"))
        .arg(clap::Arg::with_name("hardware-mapping")
            .long("led-hardware-mapping")
            .takes_value(true)
            .help("Name of the hardware mapping used"))
        .arg(clap::Arg::with_name("pwm-bits")
            .long("led-pwm-bits")
            .takes_value(true)
            .validator(regex_validator!(r"^\d+$"))
            .help("Sets the number of PWM cycles performed. More bits equal better colors at the cost of refresh speed"))
        .arg(clap::Arg::with_name("pwm-lsb-nanoseconds")
            .long("led-pwm-lsb-nanoseconds")
            .takes_value(true)
            .validator(regex_validator!(r"^\d+$"))
            .help("The on-time in the lowest significant bit in nanoseconds. Higher numbers provide better quality (more accurate color, less ghosting) at the cost of the refresh rate"))
        .arg(clap::Arg::with_name("pwm-dither-bits")
            .long("led-pwm-dither-bits")
            .help("The lower bits can be time-dithered for higher refresh rate"))
        .arg(clap::Arg::with_name("scan-mode")
            .long("led-scan-mode")
            .takes_value(true)
            .possible_values(&["progressive", "interlaced", "0", "1"])
            .help("This switches between progressive and interlaced scanning. The latter might look be a little nicer when you have a very low refresh rate"))
        .arg(clap::Arg::with_name("row-addr-type")
            .long("led-row-addr-type")
            .takes_value(true)
            .possible_values(&["direct", "ab", "0", "1"])
            .help(""))
        .arg(clap::Arg::with_name("multiplexing")
            .long("led-multiplexing")
            .takes_value(true)
            .possible_values(&["direct", "stripe", "checker", "spiral", "strip", "0", "1", "2", "3", "4"])
            .help("Outdoor panels have different multiplexing which allows them to be faster and brighter, so by default their output looks jumbled up. They require some pixel-mapping of which there are a few types you can try"))
        .arg(clap::Arg::with_name("rgb-sequence")
            .long("led-rgb-sequence")
            .takes_value(true)
            .default_value("RGB")
            .validator(regex_validator!(r"^[RGB]{3}$"))
            .help("Allows swapping of subpixels"))
}

pub fn from_command(args: &clap::ArgMatches, gargs: &GlobalArgs) -> io::Result<FromCommand> {
    unsafe {
        let mut options: RGBLedMatrixOptions = mem::zeroed();

        let (width, height) = gargs.dimensions_2d()?;
        let chain_length = args.value_of("chain").map(|s| s.parse().unwrap());
        let cols = args.value_of("cols").map(|s| s.parse().unwrap());
        let parallel = args.value_of("parallel").map(|s| s.parse().unwrap());
        let rows = args.value_of("rows").map(|s| s.parse().unwrap());
        // X = cols * chain_length
        let (calc_cols, calc_chain_length) = match (cols, chain_length) {
            (Some(c), Some(l)) => (c, l),
            (Some(c), None) => (c, width as i32 / c),
            (None, Some(l)) => (width as i32 / l, l),
            (None, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Either --chain or --cols must be set",
                ));
            }
        };
        options.cols = calc_cols;
        options.chain_length = calc_chain_length;
        // Y = rows * parallel
        let (calc_rows, calc_parallel) = match (rows, parallel) {
            (Some(r), Some(p)) => (r, p),
            (Some(r), None) => (r, height as i32 / r),
            (None, Some(p)) => (height as i32 / p, p),
            (None, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Either --rows or --parallel must be set",
                ));
            }
        };
        options.rows = calc_rows;
        options.parallel = calc_parallel;

        let hwmap = args
            .value_of("hardware-mapping")
            .map(|s| CString::new(s).unwrap());
        if let Some(s) = &hwmap {
            options.hardware_mapping = s.as_ptr() as _;
        }
        if let Some(v) = args.value_of("pwm-bits") {
            let b = v
                .parse()
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
            options.pwm_bits = b;
        }
        if let Some(v) = args.value_of("pwm-lsb-nanoseconds") {
            let ns = v
                .parse()
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
            options.pwm_lsb_nanoseconds = ns;
        }
        options.pwm_dither_bits = match args.is_present("pwm-dither-bits") {
            true => 1,
            false => 0,
        };
        if let Some(v) = args.value_of("scan-mode") {
            options.scan_mode = match v {
                "0" | "progressive" => 0,
                "1" | "interlaced" => 1,
                _ => unreachable!(),
            };
        }
        if let Some(v) = args.value_of("row-addr-type") {
            options.row_address_type = match v {
                "0" | "direct" => 0,
                "1" | "ab" => 1,
                _ => unreachable!(),
            };
        }
        if let Some(v) = args.value_of("multiplexing") {
            options.multiplexing = match v {
                "0" | "direct" => 0,
                "1" | "stripe" => 1,
                "2" | "checker" => 2,
                "3" | "spiral" => 3,
                "4" | "strip" => 4,
                _ => unreachable!(),
            };
        }
        let rgbseq = args
            .value_of("rgb-sequence")
            .map(|s| CString::new(s).unwrap());
        if let Some(s) = &rgbseq {
            options.led_rgb_sequence = s.as_ptr() as _;
        }

        let led_matrix = led_matrix_create_from_options(&options, &0, ptr::null());
        if led_matrix.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "could not initialize LED Matrix driver",
            ));
        }
        let backbuffer = led_matrix_create_offscreen_canvas(led_matrix);
        Ok(FromCommand::Output(Box::new(LedMatrix {
            led_matrix,
            backbuffer,
            width,
            height,
        })))
    }
}

#[repr(C, packed)]
struct RGBLedMatrixOptions {
    hardware_mapping: *const u8,
    rows: i32,
    cols: i32,
    chain_length: i32,
    parallel: i32,
    pwm_bits: i32,
    pwm_lsb_nanoseconds: i32,
    pwm_dither_bits: i32,
    brightness: i32,
    scan_mode: i32,
    row_address_type: i32,
    multiplexing: i32,
    led_rgb_sequence: *const u8,
    pixel_mapper_config: *const u8,
    bitfield: u8,
    // disable_hardware_pulsing
    // show_refresh_rate
    // inverse_colors
}

enum RGBLedMatrix {}

enum LedCanvas {}

extern "C" {
    fn led_matrix_create_from_options(
        options: *const RGBLedMatrixOptions,
        argc: *const i32,
        argv: *const *mut *mut u8,
    ) -> *mut RGBLedMatrix;
    fn led_matrix_delete(matrix: *mut RGBLedMatrix);
    fn led_matrix_get_canvas(matrix: *mut RGBLedMatrix) -> *mut LedCanvas;
    fn led_matrix_create_offscreen_canvas(matrix: *mut RGBLedMatrix) -> *mut LedCanvas;
    fn led_matrix_swap_on_vsync(
        matrix: *mut RGBLedMatrix,
        canvas: *mut LedCanvas,
    ) -> *mut LedCanvas;
    fn led_canvas_set_pixel(canvas: *mut LedCanvas, x: i32, y: i32, r: u8, g: u8, b: u8);
    fn led_canvas_clear(canvas: *mut LedCanvas);
}

use crate::device::*;
use librgbmatrix_sys::*;
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

pub fn command() -> clap::Command {
    clap::Command::new("rpi-led-matrix")
        .about("Hzeller's Raspberry Pi LED Matrix library")
        .after_help("For a detailed guide and caveats, please refer to https://github.com/hzeller/rpi-rgb-led-matrix for more information")
        .arg(clap::arg!(--"led-rows" <value> "The number of rows supported by the display, e.g. 32 or 16. The combined height of the display is rows * parallel")
            .required_unless_present("led-parallel")
            .value_parser(clap::value_parser!(i32)))
        .arg(clap::arg!(--"led-cols" <value> "The number of columns per panel. The combined width of the display is cols * chain")
            .required_unless_present("led-chain")
            .value_parser(clap::value_parser!(i32)))
        .arg(clap::arg!(--"led-chain" <value> "The number of panels daisy chained together")
            .required_unless_present("led-cols")
            .value_parser(clap::value_parser!(i32)))
        .arg(clap::arg!(--"led-parallel" <value> "The number of displays that are being driven in parallel")
            .required_unless_present("led-rows")
            .default_value("1")
            .value_parser(clap::value_parser!(i32)))
        .arg(clap::arg!(--"led-hardware-mapping" <value> "Name of the hardware mapping used"))
        .arg(clap::arg!(--"led-pwm-bits" <value> "Sets the number of PWM cycles performed. More bits equal better colors at the cost of refresh speed")
            .value_parser(clap::value_parser!(i32)))
        .arg(clap::arg!(--"led-pwm-lsb-nanoseconds" <value> "The on-time in the lowest significant bit in nanoseconds. Higher numbers provide better quality (more accurate color, less ghosting) at the cost of the refresh rate")
            .value_parser(clap::value_parser!(i32)))
        .arg(clap::arg!(--"ledpwm-dither-bits" "The lower bits can be time-dithered for higher refresh rate"))
        .arg(clap::arg!(--"led-scan-mode" <value> "This switches between progressive and interlaced scanning. The latter might look be a little nicer when you have a very low refresh rate")
            .value_parser(["progressive", "interlaced", "0", "1"]))
        .arg(clap::arg!(--"led-row-addr-type" <value>)
            .value_parser(["direct", "ab", "0", "1"]))
        .arg(clap::arg!(--"led-multiplexing" <value> "Outdoor panels have different multiplexing which allows them to be faster and brighter, so by default their output looks jumbled up. They require some pixel-mapping of which there are a few types you can try")
            .value_parser(["direct", "stripe", "checker", "spiral", "strip", "0", "1", "2", "3", "4"]))
        .arg(clap::arg!(--"led-rgb-sequence" <value> "Allows swapping of subpixels")
            .default_value("RGB"))
}

pub fn from_command(args: &clap::ArgMatches, gargs: &GlobalArgs) -> io::Result<FromCommand> {
    unsafe {
        let mut options: RGBLedMatrixOptions = mem::zeroed();

        let dimensions = gargs.dimensions()?;
        let chain_length = args.get_one::<i32>("led-chain").copied();
        let cols = args.get_one::<i32>("led-cols").copied();
        let parallel = args.get_one::<i32>("led-parallel").copied();
        let rows = args.get_one::<i32>("led-rows").copied();
        // X = cols * chain_length
        let (calc_cols, calc_chain_length) = match (cols, chain_length) {
            (Some(c), Some(l)) => (c, l),
            (Some(c), None) => (c, dimensions.w as i32 / c),
            (None, Some(l)) => (dimensions.w as i32 / l, l),
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
            (Some(r), None) => (r, dimensions.h as i32 / r),
            (None, Some(p)) => (dimensions.w as i32 / p, p),
            (None, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Either --rows or --parallel must be set",
                ));
            }
        };
        options.rows = calc_rows;
        options.parallel = calc_parallel;

        if let Some(s) = args.get_one::<CString>("led-hardware-mapping") {
            options.hardware_mapping = s.as_ptr() as _;
        }
        if let Some(b) = args.get_one::<i32>("led-pwm-bits") {
            options.pwm_bits = *b;
        }
        if let Some(ns) = args.get_one::<i32>("led-pwm-lsb-nanoseconds") {
            options.pwm_lsb_nanoseconds = *ns;
        }
        options.pwm_dither_bits = match args.get_flag("led-pwm-dither-bits") {
            true => 1,
            false => 0,
        };
        if let Some(v) = args.get_one::<String>("led-scan-mode") {
            options.scan_mode = match v.as_str() {
                "0" | "progressive" => 0,
                "1" | "interlaced" => 1,
                _ => unreachable!(),
            };
        }
        if let Some(v) = args.get_one::<String>("led-row-addr-type") {
            options.row_address_type = match v.as_str() {
                "0" | "direct" => 0,
                "1" | "ab" => 1,
                _ => unreachable!(),
            };
        }
        if let Some(v) = args.get_one::<String>("led-multiplexing") {
            options.multiplexing = match v.as_str() {
                "0" | "direct" => 0,
                "1" | "stripe" => 1,
                "2" | "checker" => 2,
                "3" | "spiral" => 3,
                "4" | "strip" => 4,
                _ => unreachable!(),
            };
        }
        if let Some(s) = args.get_one::<CString>("led-rgb-sequence") {
            options.led_rgb_sequence = s.as_ptr() as _;
        }

        let led_matrix = led_matrix_create_from_options(&mut options, &mut 0, ptr::null_mut());
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
            width: dimensions.w,
            height: dimensions.h,
        })))
    }
}

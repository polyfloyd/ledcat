use std::io::Write;
use clap;
use ::device::*;


pub struct AnsiDisplay {
    width: usize,
    height: usize,
}

impl Output for AnsiDisplay {
    fn color_correction(&self) -> Correction {
        Correction::none()
    }

    fn output_frame(&mut self, frame: &[Pixel]) -> io::Result<()> {
        // A buffer is used so frames can be written in one go, significantly improving
        // performance.
        let mut buf = Vec::new();

        // Clear the screen and any previous frame with it.
        write!(buf, "\x1b[3J\x1b[H\x1b[2J")?;

        // Two pixels are rendered at once using the Upper Half Block character. The top half is
        // colored with the foreground color while the lower half uses the background. This neat
        // trick allows us to render square pixels with a higher density than combining two
        // rectangular characters.
        for y in 0..self.height / 2 + (self.height & 1) {
            for x in 0..self.width {
                let pix_hi = &frame[y * 2 * self.width + x];
                let pix_lo = frame.get((y * 2 + 1) * self.width + x);
                // Set the foreground color.
                write!(buf, "\x1b[38;2;{};{};{}m", pix_hi.r, pix_hi.g, pix_hi.b)?;
                // Set the background color.
                if let Some(pix_lo) = pix_lo {
                    write!(buf, "\x1b[48;2;{};{};{}m", pix_lo.r, pix_lo.g, pix_lo.b)?;
                } else {
                    write!(buf, "\x1b[48;2;0m")?;
                }
                write!(buf, "\u{2580}")?;
            }
            // Reset to the default background color and jump to the next line.
            write!(buf, "\x1b[0m\n")?;
        }

        io::stdout().write_all(&buf)
    }
}

pub fn command<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("show")
        .about("Visualize 2D frames using a text based display")
}

pub fn from_command(_: &clap::ArgMatches, gargs: &GlobalArgs) -> io::Result<FromCommand> {
    let (width, height) = gargs.dimensions_2d()?;
    Ok(FromCommand::Output(Box::new(AnsiDisplay {
        width,
        height,
    })))
}

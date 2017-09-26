use std::fs;
use std::os::unix::io::AsRawFd;
use std::path;
use nix::sys::termios;
use regex;
use driver;

pub fn open<P: AsRef<path::Path>>(path: P, baudrate: u32) -> Result<fs::File, driver::Error> {
    let tty = fs::OpenOptions::new()
        .write(true)
        .read(true)
        .open(path)?;
    let fd = tty.as_raw_fd();
    let mut tio = termios::tcgetattr(fd)?;
    tio.input_flags &= !(termios::ICRNL|termios::BRKINT);
    tio.output_flags &= !(termios::OPOST|termios::ONLCR);
    tio.local_flags &= !(termios::ICANON|termios::ISIG|termios::ECHO);
    termios::cfsetspeed(&mut tio, map_baudrate(baudrate))?;
    termios::tcsetattr(fd, termios::SetArg::TCSANOW, &tio)?;
    Ok(tty)
}

pub fn is_serial(path: &path::Path) -> bool {
    let devs = regex::RegexSet::new(&[r"^/dev/tty"])
        .unwrap();
    devs.is_match(path.to_str().unwrap())
}

fn map_baudrate(b: u32) -> termios::BaudRate {
    match b {
        b if b > 4000000 => termios::BaudRate::B4000000,
        b if b > 3500000 => termios::BaudRate::B3500000,
        b if b > 3000000 => termios::BaudRate::B3000000,
        b if b > 2500000 => termios::BaudRate::B2500000,
        b if b > 2000000 => termios::BaudRate::B2000000,
        b if b > 1500000 => termios::BaudRate::B1500000,
        b if b > 1152000 => termios::BaudRate::B1152000,
        b if b > 1000000 => termios::BaudRate::B1000000,
        b if b > 921600 => termios::BaudRate::B921600,
        b if b > 576000 => termios::BaudRate::B576000,
        b if b > 500000 => termios::BaudRate::B500000,
        b if b > 460800 => termios::BaudRate::B460800,
        b if b > 230400 => termios::BaudRate::B230400,
        b if b > 115200 => termios::BaudRate::B115200,
        b if b > 57600 => termios::BaudRate::B57600,
        b if b > 38400 => termios::BaudRate::B38400,
        b if b > 19200 => termios::BaudRate::B19200,
        b if b > 9600 => termios::BaudRate::B9600,
        b if b > 4800 => termios::BaudRate::B4800,
        b if b > 2400 => termios::BaudRate::B2400,
        b if b > 1800 => termios::BaudRate::B1800,
        b if b > 1200 => termios::BaudRate::B1200,
        b if b > 600 => termios::BaudRate::B600,
        b if b > 300 => termios::BaudRate::B300,
        b if b > 200 => termios::BaudRate::B200,
        b if b > 150 => termios::BaudRate::B150,
        b if b > 134 => termios::BaudRate::B134,
        b if b > 110 => termios::BaudRate::B110,
        b if b > 75 => termios::BaudRate::B75,
        b if b > 50 => termios::BaudRate::B50,
        _ => termios::BaudRate::B0,
    }
}

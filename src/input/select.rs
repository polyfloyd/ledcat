use std::fs;
use std::io;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::sync::mpsc;
use std::thread;
use std::time;
use libc;

/// Tells readers how to handle excessive data if more is produced from the inputs than is being
/// read.
#[derive(Clone)]
pub enum Consume {
    /// Only pull data from the selected input. All other streams are blocked and their data is
    /// kept until needed.
    Single,
    /// Keeps pulling from all streams regardless of whether the data is actually being presented
    /// to the reader. Be sure to idle between frames in the producer to make sure no frames are
    /// dropped.
    All(time::Duration),
}

pub struct Reader {
    consume:   Consume,
    receivers: Vec<mpsc::Receiver<Vec<u8>>>,
    current:   Option<io::Cursor<Vec<u8>>>,
}

impl Reader {

    pub fn from_files(filenames: Vec<&str>, switch_after: usize, consume: Consume) -> io::Result<Reader> {
        let files: io::Result<Vec<fs::File>> = filenames.into_iter().map(|filename| {
            let mut open_opts = fs::OpenOptions::new();
            open_opts.read(true);

            let is_fifo = if cfg!(unix) {
                try!(fs::metadata(filename)).file_type().is_fifo()
            } else { false };
            if is_fifo {
                // A FIFO will block the call to open() until the other end has been opened. This
                // means that when multiple FIFO's are used, they all have to be open at once
                // before this program can continue.
                // Opening the file with O_NONBLOCK will ensure that we don't have to wait.
                open_opts.custom_flags(libc::O_NONBLOCK);
            }

            let file = try!(open_opts.open(filename));

            if is_fifo {
                unsafe {
                    // Now unset the O_NONBLOCK flag so reads will block again.
                    let fd = file.as_raw_fd();
                    let opts = libc::fcntl(fd, libc::F_GETFL);
                    assert!(opts & libc::O_NONBLOCK > 0);
                    if opts < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    if libc::fcntl(fd, libc::F_SETFL, opts & !libc::O_NONBLOCK) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
            }

            Ok(file)
        }).collect();
        Ok(Reader::from(try!(files), switch_after, consume))
    }

    pub fn from<R>(inputs: Vec<R>, switch_after: usize, consume: Consume) -> Reader
        where R: io::Read + Send + 'static {
        assert_ne!(inputs.len(), 0);

        let receivers = inputs.into_iter().map(|mut input| {
            let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
            let consume = consume.clone();
            thread::spawn(move || {
                loop {
                    let mut buf = Vec::new();
                    buf.resize(switch_after, 0);
                    if let Err(_) = input.read_exact(&mut buf) {
                        thread::sleep(time::Duration::new(0, 1_000_000)); // TODO
                        continue;
                    }
                    match consume {
                        Consume::Single => {
                            if let Err(_) = tx.send(buf) {
                                break;
                            }
                        },
                        Consume::All(interval) => {
                            if let Err(mpsc::TrySendError::Disconnected(_)) = tx.try_send(buf) {
                                break;
                            }
                            thread::sleep(interval);
                        },
                    };
                }
            });
            rx
        }).collect();

        Reader {
            consume:   consume,
            receivers: receivers,
            current:   None,
        }
    }

}

impl io::Read for Reader {

    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.current.is_none() {
                let buf = self.receivers.iter().fold(None, |buf, rx| {
                    match self.consume {
                        Consume::Single => buf.or_else(|| rx.try_recv().ok()),
                        Consume::All(_) => buf.or(rx.try_recv().ok()),
                    }
                });
                self.current = buf.map(io::Cursor::new);
            }

            if self.current.is_some() {
                let mut cur = self.current.take().unwrap();
                let nread = cur.read(&mut buf).unwrap();
                self.current = if nread == 0 { None } else { Some(cur) };
                if nread > 0 {
                    return Ok(nread);
                }
            }

            thread::sleep(time::Duration::new(0, 1_000_000)); // TODO
        }
    }

}

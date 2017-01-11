use std::fs;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time;

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

    pub fn from_files(filenames: Vec<&str>, switch_after: usize, consume: Consume) -> io::Result<Box<io::Read>> {
        let mut files = Vec::new();
        for filename in filenames {
            files.push(try!(fs::OpenOptions::new().read(true).open(filename)));
        }
        Ok(Reader::from(files, switch_after, consume))
    }

    pub fn from<R>(mut inputs: Vec<R>, switch_after: usize, consume: Consume) -> Box<io::Read>
        where R: io::Read + Send + 'static {
        assert_ne!(inputs.len(), 0);
        if inputs.len() == 1 {
            return Box::from(inputs.remove(0));
        }

        let receivers = inputs.drain(..).map(|mut input| {
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

        Box::from(Reader{
            consume:   consume,
            receivers: receivers,
            current:   None,
        })
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

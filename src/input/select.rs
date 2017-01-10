use std::fs;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time;

pub struct Reader {
    receivers: Vec<mpsc::Receiver<Vec<u8>>>,
    current:   Option<io::Cursor<Vec<u8>>>,
}

impl Reader {

    pub fn from_files(filenames: Vec<&str>, switch_after: usize) -> io::Result<Box<io::Read>> {
        let mut files = Vec::new();
        for filename in filenames {
            files.push(try!(fs::OpenOptions::new().read(true).open(filename)));
        }
        Ok(Reader::from(files, switch_after))
    }

    pub fn from<R: io::Read + Send + 'static>(mut inputs: Vec<R>, switch_after: usize) -> Box<io::Read> {
        match inputs.len() {
            0 => panic!("No inputs specified"),
            1 => return Box::from(inputs.remove(0)),
            _ => (),
        };

        let receivers = inputs.drain(..).map(|mut input| {
            let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
            thread::spawn(move || {
                loop { // TODO: wait instead of looping
                    let mut buf = Vec::new();
                    buf.resize(switch_after, 0);
                    if let Err(_) = input.read_exact(&mut buf) {
                        thread::sleep(time::Duration::new(0, 1_000_000));
                        continue;
                    }
                    if let Err(mpsc::TrySendError::Disconnected(_)) = tx.try_send(buf) {
                        break;
                    }
                }
            });
            rx
        }).collect();

        Box::from(Reader{
            receivers: receivers,
            current:   None,
        })
    }

}

impl io::Read for Reader {

    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        loop { // TODO: wait instead of looping
            if self.current.is_none() {
                let buf = self.receivers.iter().fold(None, |buf, rx| {
                    buf.or(rx.try_recv().ok())
                });
                self.current = buf.map(io::Cursor::new);
            }

            if self.current.is_some() {
                let mut cur = self.current.take().unwrap();
                let rs = cur.read(&mut buf);
                self.current = match rs {
                    Err(e)    => panic!("Unexpected error from cursor: {}", e),
                    Ok(nread) => if nread == 0 {
                        None
                    } else {
                        Some(cur)
                    },
                };
                if let Ok(nread) = rs {
                    if nread > 0 {
                        return rs;
                    }
                }
            }
            thread::sleep(time::Duration::new(0, 1_000_000));
        }
    }

}

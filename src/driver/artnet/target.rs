use std::borrow::Cow;
use std::fs;
use std::io::{self, BufRead};
use std::net;
use std::path;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time;

pub trait Target: Send {
    fn addresses(&self) -> Cow<[net::SocketAddr]>;
}

impl Target for Vec<net::SocketAddr> {
    fn addresses(&self) -> Cow<[net::SocketAddr]> {
        Cow::Borrowed(self)
    }
}

pub struct Broadcast {}

impl Target for Broadcast {
    fn addresses(&self) -> Cow<[net::SocketAddr]> {
        let ip = net::Ipv4Addr::new(255, 255, 255, 255);
        let addrs = vec![net::SocketAddrV4::new(ip, super::PORT).into()];
        Cow::Owned(addrs)
    }
}

pub struct ListFile {
    cache: Arc<RwLock<Vec<net::SocketAddr>>>,
}

impl ListFile {
    pub fn new<T: Into<path::PathBuf>>(p: T) -> ListFile {
        let path = p.into();
        let cache = Arc::new(RwLock::new(Vec::new()));

        let cache_weak = Arc::downgrade(&cache);
        thread::spawn(move || {
            macro_rules! try_or_continue {
                ($expr:expr) => {{
                    match $expr {
                        Ok(t) => t,
                        Err(_) => continue,
                    }
                }};
            }

            let mut prev_mod_time = None;
            loop {
                let meta = try_or_continue!(fs::metadata(&path));
                let mod_time = try_or_continue!(meta.modified());
                let reload = prev_mod_time != Some(mod_time);
                prev_mod_time = Some(mod_time);

                if reload {
                    let cache = match cache_weak.upgrade() {
                        Some(c) => c,
                        None => return,
                    };
                    let mut v = cache.write().unwrap();
                    v.clear();

                    let file = try_or_continue!(fs::File::open(&path));
                    let addrs = io::BufReader::new(file)
                        .lines()
                        .filter_map(|rs| rs.ok())
                        .filter_map(|line| {
                            line.parse().ok().or_else(|| {
                                line.parse()
                                    .ok()
                                    .map(|ip| net::SocketAddr::new(ip, super::PORT))
                            })
                        });
                    v.extend(addrs);
                    v.dedup();
                }

                thread::sleep(time::Duration::new(1, 0));
            }
        });

        ListFile { cache }
    }
}

impl Target for ListFile {
    fn addresses(&self) -> Cow<[net::SocketAddr]> {
        Cow::Owned(self.cache.read().unwrap().clone())
    }
}

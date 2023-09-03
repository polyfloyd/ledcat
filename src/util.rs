macro_rules! io_err {
    ($expr:expr) => {
        $expr.map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    };
}

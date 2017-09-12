macro_rules! regex_validator {
    ($expression:expr) => ({
        use regex::Regex;
        let ex = Regex::new($expression).unwrap();
        move |val: String| {
            if ex.is_match(val.as_str()) {
                Ok(())
            } else {
                Err(format!("\"{}\" does not match {}", val, ex))
            }
        }
    })
}

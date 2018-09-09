#![cfg(test)]
extern crate assert_cli;

#[test]
#[cfg(not(feature = "ci"))]
fn calling_without_args() {
    assert_cli::Assert::main_binary()
        .fails()
        .stderr()
        .contains("USAGE")
        .stdout()
        .is("")
        .unwrap();
}

#[test]
#[cfg(not(feature = "ci"))]
fn calling_generic_pipe() {
    let data = "\x00\x01\x02\x03\x04\x05\x06\x07\x08";
    assert_cli::Assert::main_binary()
        .with_args(&["--geometry", "3", "--driver", "none", "generic"])
        .stdin(data)
        //.stderr().is("") TODO: https://github.com/killercup/assert_cli/issues/45
        .stdout().is(data)
        .unwrap();
}

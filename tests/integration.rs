#![cfg(test)]
extern crate assert_cli;

#[test]
fn calling_without_args() {
    assert_cli::Assert::main_binary()
        .fails()
        .and()
        .stderr().contains("USAGE")
        .stdout().is("")
        .unwrap();
}

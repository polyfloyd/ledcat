use std::process::Command;
use std::path::*;
use std::env;

// The RPI LED Matrix library is stored using a subtree, see:
//   https://www.atlassian.com/blog/git/alternatives-to-git-submodule-git-subtree
//
// To update, run;
//   git subtree pull --prefix components/rpi-led-matrix https://github.com/hzeller/rpi-rgb-led-matrix.git master --squash

fn main () {
    if env::var("CARGO_FEATURE_CI").is_ok() {
        return;
    }

    Command::new("make")
        .current_dir("./components/rpi-rgb-led-matrix")
        .status().unwrap();

    let lib_dir = Path::new("./components/rpi-rgb-led-matrix/lib")
        .canonicalize().unwrap()
        .to_str().unwrap()
        .to_string();
    println!("cargo:rustc-link-search={}", lib_dir);
    println!("cargo:rustc-link-lib=static=rgbmatrix");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}

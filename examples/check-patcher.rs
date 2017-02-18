extern crate bsdiff;

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Read, Write, Cursor};
use std::fmt;

use bsdiff::patch;

fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut contents = Vec::new();
    File::open(path)?.read_to_end(&mut contents)?;
    Ok(contents)
}

fn main() {
    let a_name = "avian_linux";
    let b_name = "avian_pr_linux";

    let a = load(format!("tests/{}", a_name)).unwrap();
    let b = load(format!("tests/{}", b_name)).unwrap();
    let a_to_b = load(format!("diffs/{}-to-{}.diff", a_name, b_name)).unwrap();

    let mut result = Vec::new();

    patch::apply(&a_to_b, Cursor::new(a), &mut result).unwrap();

    assert_eq!(b, result);
}
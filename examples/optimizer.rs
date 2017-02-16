extern crate bsdiff;

use std::path::Path;
use std::fs::File;
use std::io::{self, Read};

use bsdiff::diff::SuffixArray;

fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut contents = Vec::new();
    File::open(path)?.read_to_end(&mut contents)?;
    Ok(contents)
}

fn main() {
    let a = load("tests/avian_linux").unwrap();
    let b = load("tests/avian_pr_linux").unwrap();

    let index = SuffixArray::new(&a);

    let diff = index.diff_to(&b);

    println!("diff {:?}", diff);
}
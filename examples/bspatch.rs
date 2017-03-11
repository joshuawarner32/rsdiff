extern crate rsdiff;

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Read, Write, Cursor};
use std::{env, fmt};

use rsdiff::patch;

fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut contents = Vec::new();
    File::open(path)?.read_to_end(&mut contents)?;
    Ok(contents)
}

fn main() {
    let args = env::args().collect::<Vec<_>>();

    if args.len() != 4 {
        panic!("Expected 3 arguments");
    }

    let old = File::open(&args[1]).unwrap();
    let new = File::create(&args[2]).unwrap();
    let patch = load(&args[3]).unwrap();

    patch::apply(&patch, old, new).unwrap();
}

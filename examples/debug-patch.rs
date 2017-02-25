extern crate rsdiff;

use std::path::Path;
use std::fs::File;
use std::io::{self, Read, Cursor};
use std::env;

use rsdiff::Header;

fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut contents = Vec::new();
    File::open(path)?.read_to_end(&mut contents)?;
    Ok(contents)
}

fn main() {
    let ref arg = env::args().collect::<Vec<_>>()[1];

    let patch = load(arg).unwrap();

    println!("size: {}", patch.len());
    let h = Header::read(&patch).unwrap();
    println!("{:?}", h);
    println!("extra size: {}",
        patch.len()
            - 32
            - h.compressed_commands_size as usize
            - h.compressed_delta_size as usize);
}
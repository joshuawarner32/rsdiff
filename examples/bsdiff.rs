extern crate rsdiff;

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Read, Write, Cursor};
use std::{env, fmt};

use rsdiff::diff::{Cache, Index, generate_full_patch};

fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut contents = Vec::new();
    File::open(path)?.read_to_end(&mut contents)?;
    Ok(contents)
}

struct FileCache {
    path: PathBuf
}

impl FileCache {
    fn new(path: PathBuf) -> FileCache {
        fs::create_dir_all(&path).unwrap();
        FileCache {
            path: path
        }
    }
}

struct Hex<'a>(pub &'a [u8]);

impl<'a> fmt::Display for Hex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0.iter() {
            try!(write!(f, "{:02x}", byte));
        }
        Ok(())
    }
}

impl<'a> Cache for &'a mut FileCache {
    type Read = File;
    type Write = File;

    fn get(&self, digest: &[u8; 20]) -> io::Result<Option<Self::Read>> {
        match File::open(self.path.join(format!("{}", Hex(digest)))) {
            Ok(read) =>
                Ok(Some(read)),
            Err(ref e) if e.kind() == io::ErrorKind::NotFound =>
                Ok(None),
            Err(e) =>
                Err(e)
        }
    }

    fn get_writer(&self, digest: &[u8; 20]) -> io::Result<Self::Write> {
        File::create(self.path.join(format!("{}", Hex(digest))))
    }
}

fn main() {
    let args = env::args().collect::<Vec<_>>();

    if args.len() != 4 {
        panic!("Expected 3 arguments");
    }

    let old = load(&args[1]).unwrap();
    let new = load(&args[2]).unwrap();

    let mut cache = FileCache::new(PathBuf::from(".cache"));

    let old_index = Index::from_cache_or_compute(&mut cache, old).unwrap();

    let patch_data = generate_full_patch(&old_index, &new);

    File::create(&args[3]).unwrap().write_all(&patch_data).unwrap();
}

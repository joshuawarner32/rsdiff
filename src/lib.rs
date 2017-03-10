extern crate byteorder;
extern crate bzip2;
extern crate zstd;
extern crate sha1;

mod core;

pub mod patch;
pub mod diff;

pub use core::Header;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::str;

    #[test]
    fn test_identity_patch() {
        let buf = b"this is a test";
        let patch = diff::generate_identity_patch(buf.len() as u64);
        
        let mut new = Vec::new();
        let mut old = Cursor::new(buf);

        patch::apply(&patch, &mut old, &mut new).unwrap();

        assert_eq!(&buf[..], &new[..]);
    }

    #[test]
    fn test_idempotent_patch() {
        let buf = b"this is a test";
        let patch = diff::generate_idempotent_patch(buf);

        let examples = [
            "",
            "this is a test",
            "1234",
            "\0"
        ];
        
        for example in examples.iter() {
            let mut new = Vec::new();
            let mut old = Cursor::new(example);

            patch::apply(&patch, &mut old, &mut new).unwrap();

            assert_eq!(&buf[..], &new[..]);
        }
    }

    #[test]
    fn test_simple_patch() {
        let buf = b"this is a test";
        let buf2 = b"this is really a cool test";
        let index = diff::Index::compute(buf.to_vec());
        let patch = diff::generate_full_patch(&index, &buf2[..]);
        
        let mut new = Vec::new();
        let mut old = Cursor::new(buf);

        patch::apply(&patch, &mut old, &mut new).unwrap();

        assert_eq!(&buf2[..], &new[..]);
    }

    #[test]
    fn test_full_patch() {
        let buf = b"this is a test 12345678 test";
        let buf2 = b"this is really a cool uftu 12345678 uftu";
        let index = diff::Index::compute(buf.to_vec());
        let patch = diff::generate_full_patch(&index, &buf2[..]);
        
        let mut new = Vec::new();
        let mut old = Cursor::new(buf);

        println!("done making patch");
        patch::apply(&patch, &mut old, &mut new).unwrap();

        assert_eq!(str::from_utf8(buf2).unwrap(), str::from_utf8(&new).unwrap());
    }
}
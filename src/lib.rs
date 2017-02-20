extern crate byteorder;
extern crate bzip2;
extern crate zstd;
extern crate sha1;

mod core;

pub mod patch;
pub mod diff;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
}
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
}
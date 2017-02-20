use std::io::{self, Read, Write};

pub struct Header {
    // NOTE: there's a non-stored field: magic (always b"BSDIFF40")

    pub compressed_commands_size: u64,
    pub compressed_delta_size: u64,

    // NOTE: the compressed_extra_size is implicitly the size of the entire
    // remainder of the patch file, after the compressed "delta" data.

    pub new_file_size: u64,
}

impl Header {
    pub fn read(buf: &[u8]) -> io::Result<Header> {
        if &buf[0..8] != b"BSDIFF40" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Bad header"));
        }

        Ok(Header {
            compressed_commands_size: read_offset(&buf[8..8+8]) as u64,
            compressed_delta_size: read_offset(&buf[16..8+16]) as u64,
            new_file_size: read_offset(&buf[24..8+24]) as u64,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub bytewise_add_size: u64,
    pub extra_append_size: u64,
    pub oldfile_seek_offset: i64,
}

pub struct CommandReader<R> {
    inner: R
}

impl<R> CommandReader<R>
    where R: Read
{
    pub fn new(inner: R) -> CommandReader<R> {
        CommandReader {
            inner: inner
        }
    }
}

pub fn read_offset(buf: &[u8]) -> i64 {
    let mut y = (buf[7] & 0x7F) as i64;

    for i in 0..7 {
        y = y * 256;
        y += buf[6 - i] as i64;
    }

    if (buf[7] & 0x80) != 0 {
        y = -y;
    }

    y
}

pub fn write_offset(buf: &mut [u8], x: i64) {
    let mut y = if x < 0 { x.wrapping_neg() } else { x };

    for i in 0..8 {
        buf[i] = y as u8;
        y -= buf[i] as i64;
        y /= 256;
    }

    if x < 0 {
        buf[7] |= 0x80;
    }
}

impl<R> Iterator for CommandReader<R>
    where R: Read
{
    type Item = io::Result<Command>;

    fn next(&mut self) -> Option<io::Result<Command>> {
        let mut buf = [0u8; 8*3];

        let mut p = 0;
        while p < buf.len() {
            println!("loop");
            match self.inner.read(&mut buf[p..]) {
                Ok(0) => {
                    println!("1");
                    return None
                }
                Ok(size) => {
                    println!("2 => {}", size);
                    p += size
                }
                Err(e) => {
                    println!("3");
                    return Some(Err(e))
                }
            }
        }

        Some(Ok(Command {
            bytewise_add_size: read_offset(&buf[0..8]) as u64,
            extra_append_size: read_offset(&buf[8..16]) as u64,
            oldfile_seek_offset: read_offset(&buf[16..24]),
        }))
    }
}

pub struct CommandWriter<W> {
    inner: W
}

impl<W: Write> CommandWriter<W> {
    pub fn new(inner: W) -> CommandWriter<W> {
        CommandWriter {
            inner: inner
        }
    }

    pub fn write(&mut self, c: &Command) -> io::Result<()> {
        let mut buf = [0u8; 8*3];

        write_offset(&mut buf[0..8], c.bytewise_add_size as i64);
        write_offset(&mut buf[8..16], c.extra_append_size as i64);
        write_offset(&mut buf[16..24], c.oldfile_seek_offset);

        self.inner.write_all(&buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn assert_identity_encoding(tests: &[(i64)]) {
        for test in tests {
            let mut buf = [0u8; 8];

            println!("trying 0x{:x}", test);
            
            write_offset(&mut buf, *test);
            let result = read_offset(&buf);

            println!("  got 0x{:x}", result);

            assert_eq!(*test, result);
        }
    }

    #[test]
    fn test_read_write_offset_roundtrip() {
        assert_identity_encoding(&[
            0, 1, -1, 2, -2, 3, -3,
            127, -127, 128, -128, 129, -129,
            255, -255, 256, -256, 257, -257,
            16383, -16383, 16384, -16384, 16385, -16385,
            65535, -65535, 65536, -65536, 65537, -65537,
            0x7ffffffffffffffe,
            0x7fffffffffffffff,
            // -0x8000000000000000, // TODO: investigate breakage
            -0x7fffffffffffffff,
        ]);
    }

    #[test]
    fn test_command_roundtrip() {
        let cmds = vec![
            Command {
                bytewise_add_size: 1,
                extra_append_size: 2,
                oldfile_seek_offset: 3,
            },
            Command {
                bytewise_add_size: 4,
                extra_append_size: 5,
                oldfile_seek_offset: 6,
            },
            Command {
                bytewise_add_size: 7,
                extra_append_size: 8,
                oldfile_seek_offset: 9,
            }
        ];

        let mut encoded = Vec::new();

        {
            let mut writer = CommandWriter::new(&mut encoded);

            for c in &cmds {
                writer.write(c).unwrap();
            }
        }

        let reader = CommandReader::new(Cursor::new(encoded));

        let result = reader.map(|e| e.unwrap()).collect::<Vec<_>>();

        assert_eq!(cmds, result);
    }
}

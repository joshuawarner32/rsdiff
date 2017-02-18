use std::io::Read;
use std::io;

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


impl<R> Iterator for CommandReader<R>
    where R: Read
{
    type Item = io::Result<Command>;

    fn next(&mut self) -> Option<io::Result<Command>> {
        let mut buf = [0u8; 8*3];

        let mut p = 0;
        loop {
            match self.inner.read(&mut buf[p..]) {
                Ok(0) => return None,
                Ok(size) => p += size,
                Err(e) => return Some(Err(e))
            }
        }

        Some(Ok(Command {
            bytewise_add_size: read_offset(&buf[0..8]) as u64,
            extra_append_size: read_offset(&buf[8..16]) as u64,
            oldfile_seek_offset: read_offset(&buf[16..24]),
        }))
    }
}

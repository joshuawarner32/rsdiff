use std::io::{self, Read, Write, Seek, Cursor, BufReader};
use std::cmp::{min, max, Ordering};
use std::ops::Range;
use std::{mem, str};

use byteorder::{LittleEndian, WriteBytesExt, ReadBytesExt};
use bzip2::write::BzEncoder;
use bzip2::bufread::BzDecoder;
use bzip2;

use diff::{
    Index,
    write_delta,
    write_zeros,
    MatchIter,
};

use patch::{
    read_paired_bufs,
    read_size_from,
};

#[derive(Debug)]
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
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Bad header: {}",
                unsafe { ::std::str::from_utf8_unchecked(&buf[0..8]) } )));
        }

        Ok(Header {
            compressed_commands_size: read_offset(&buf[8..8+8]) as u64,
            compressed_delta_size: read_offset(&buf[16..8+16]) as u64,
            new_file_size: read_offset(&buf[24..8+24]) as u64,
        })
    }

    pub fn write_to<W: Write>(&self, mut writer: W) -> io::Result<()> {
        let mut buf = [0u8; 8*4];

        buf[0..8].copy_from_slice(b"BSDIFF40");
        write_offset(&mut buf[8..16], self.compressed_commands_size as i64);
        write_offset(&mut buf[16..24], self.compressed_delta_size as i64);
        write_offset(&mut buf[24..32], self.new_file_size as i64);

        writer.write_all(&buf)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub bytewise_add_size: u64,
    pub extra_append_size: u64,
    pub oldfile_seek_offset: i64,
}

impl Command {
    pub fn write_to<W: Write>(&self, mut writer: W) -> io::Result<()> {
        let mut buf = [0u8; 8*3];

        write_offset(&mut buf[0..8], self.bytewise_add_size as i64);
        write_offset(&mut buf[8..16], self.extra_append_size as i64);
        write_offset(&mut buf[16..24], self.oldfile_seek_offset);

        writer.write_all(&buf)
    }
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
            // println!("loop");
            match self.inner.read(&mut buf[p..]) {
                Ok(0) => {
                    // println!("1");
                    return None
                }
                Ok(size) => {
                    // println!("2 => {}", size);
                    p += size
                }
                Err(e) => {
                    // println!("3");
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

struct PatchWriter {
    new_file_size: usize,
    cmds: BzEncoder<Vec<u8>>,
    delta: BzEncoder<Vec<u8>>,
    extra: BzEncoder<Vec<u8>>,
}

impl PatchWriter {
    fn new(new_file_size: usize) -> PatchWriter {
        PatchWriter {
            new_file_size: new_file_size,
            cmds: BzEncoder::new(Vec::new(), bzip2::Compression::Best),
            delta: BzEncoder::new(Vec::new(), bzip2::Compression::Best),
            extra: BzEncoder::new(Vec::new(), bzip2::Compression::Best),
        }
    }

    fn finish(self) -> Vec<u8> {
        let cmds = self.cmds.finish().unwrap();
        let delta = self.delta.finish().unwrap();
        let extra = self.extra.finish().unwrap();

        let mut patch = Vec::new();

        Header {
            compressed_commands_size: cmds.len() as u64,
            compressed_delta_size: delta.len() as u64,
            new_file_size: self.new_file_size as u64,
        }.write_to(&mut patch).unwrap();

        patch.extend(&cmds);
        patch.extend(&delta);
        patch.extend(&extra);

        patch
    }

    fn write_delta_zeros(&mut self, count: usize) {
        write_zeros(&mut self.delta, count as u64).unwrap();
    }

    fn write_delta(&mut self, old: &[u8], new: &[u8]) {
        write_delta(&mut self.delta, old, new).unwrap();
    }

    fn write_extra(&mut self, new: &[u8]) {
        self.extra.write_all(new).unwrap();
        // println!("write extra {}", new.len());
    }

    fn write_command(&mut self, cmd: &Command) {
        cmd.write_to(&mut self.cmds).unwrap();
    }
}

pub struct Patcher<DeltaR, ExtraR, OldRS, NewW> {
    delta: DeltaR,
    extra: ExtraR,
    old: OldRS,
    new: NewW,
}

impl<DeltaR, ExtraR, OldRS, NewW> Patcher<DeltaR, ExtraR, OldRS, NewW>
    where
        DeltaR: Read,
        ExtraR: Read,
        OldRS: Read+Seek,
        NewW: Write
{

    pub fn new(delta: DeltaR, extra: ExtraR, old: OldRS, new: NewW) -> Patcher<DeltaR, ExtraR, OldRS, NewW> {
        Patcher {
            delta: delta,
            extra: extra,
            old: old,
            new: new,
        }
    }

    pub fn apply(&mut self, c: &Command) -> io::Result<()> {
        self.append_delta(c.bytewise_add_size)?;
        self.append_extra(c.extra_append_size)?;
        self.seek_old(c.oldfile_seek_offset)?;
        Ok(())
    }

    pub fn append_delta(&mut self, size: u64) -> io::Result<()> {
        let new = &mut self.new;
        read_paired_bufs(size, &mut self.old, &mut self.delta, |o, d| {
            for i in 0..o.len() {
                o[i] = o[i].wrapping_add(d[i]);
            }
            new.write_all(&o)
        })
    }

    pub fn append_extra(&mut self, size: u64) -> io::Result<()> {
        let new = &mut self.new;
        read_size_from(size, &mut self.extra, |e| {
            new.write_all(&e)
        })
    }

    pub fn seek_old(&mut self, size: i64) -> io::Result<()> {
        self.old.seek(io::SeekFrom::Current(size)).map(|_|())
    }

    pub fn check_written_size(&self, _: u64) -> io::Result<()> {
        // TODO: return an error if we haven't written the expected size to the output.
        Ok(())
    }
}

pub fn generate_identity_patch(size: u64) -> Vec<u8> {
    let mut w = PatchWriter::new(size as usize);

    w.write_delta_zeros(size as usize);

    w.write_command(&Command {
        bytewise_add_size: size,
        extra_append_size: 0,
        oldfile_seek_offset: 0,
    });

    w.finish()
}

pub fn generate_idempotent_patch(desired_output: &[u8]) -> Vec<u8> {
    let mut w = PatchWriter::new(desired_output.len());

    w.write_extra(desired_output);

    w.write_command(&Command {
        bytewise_add_size: 0,
        extra_append_size: desired_output.len() as u64,
        oldfile_seek_offset: 0,
    });

    w.finish()
}

pub fn generate_full_patch(old: &Index, new: &[u8]) -> Vec<u8> {
    let mut w = PatchWriter::new(new.len());

    let mut i = 0;

    let mut k = 0;

    let mut it = MatchIter::from(old, new).peekable();


    while let Some(m) = it.next() {

        if k % 1024 == 0 {
            println!("{} / {} ({}%)", i, new.len(), i * 100 / new.len());
        }

        k += 1;

        let mm = m.matched;
        let next_old_offset = it.peek()
            .map(|m| m.matched.old_offset)
            .unwrap_or(mm.old_offset + mm.len());

        w.write_command(&Command {
            bytewise_add_size: mm.len() as u64,
            extra_append_size: m.unmatched_suffix as u64,
            oldfile_seek_offset: next_old_offset as i64 - (mm.old_offset + mm.len()) as i64,
        });

        w.write_delta(
            &old.data[mm.lower_delta_range()], 
            &new[i .. i + mm.lower_delta_len]);

        w.write_delta_zeros(mm.mid_exact_len);

        w.write_delta(
            &old.data[mm.upper_delta_range()], 
            &new[i + mm.lower_delta_len + mm.mid_exact_len .. i + mm.len()]);

        let extra_begin = i + mm.len();
        let extra_end = extra_begin + m.unmatched_suffix;

        w.write_extra(&new[extra_begin .. extra_end]);

        i = extra_end;
    }

    w.finish()
}

pub fn apply_patch<OldRS, NewW>(patch: &[u8], old: OldRS, new: NewW) -> io::Result<()>
    where
        OldRS: Read+Seek,
        NewW: Write
{
    let (header, body) = patch.split_at(32);

    let header = Header::read(&header)?;

    let (command_data, rest) = body.split_at(header.compressed_commands_size as usize);
    let (delta_data, extra_data) = rest.split_at(header.compressed_delta_size as usize);

    let command_stream = BzDecoder::new(Cursor::new(command_data));

    let commands = CommandReader::new(command_stream);

    let delta = BzDecoder::new(Cursor::new(delta_data));
    let extra = BzDecoder::new(Cursor::new(extra_data));

    let mut patcher = Patcher::new(delta, extra, old, new);

    for cmd in commands {
        // println!("cmd {:?}", cmd);
        patcher.apply(&(cmd?))?;
    }

    patcher.check_written_size(header.new_file_size)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use diff::Index;

    fn assert_identity_encoding(tests: &[(i64)]) {
        for test in tests {
            let mut buf = [0u8; 8];

            // println!("trying 0x{:x}", test);
            
            write_offset(&mut buf, *test);
            let result = read_offset(&buf);

            // println!("  got 0x{:x}", result);

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
            for c in &cmds {
                c.write_to(&mut encoded).unwrap();
            }
        }

        let reader = CommandReader::new(Cursor::new(encoded));

        let result = reader.map(|e| e.unwrap()).collect::<Vec<_>>();

        assert_eq!(cmds, result);
    }

    #[test]
    fn test_identity_patch() {
        let buf = b"this is a test";
        let patch = generate_identity_patch(buf.len() as u64);
        
        let mut new = Vec::new();
        let mut old = Cursor::new(buf);

        apply_patch(&patch, &mut old, &mut new).unwrap();

        assert_eq!(&buf[..], &new[..]);
    }

    #[test]
    fn test_idempotent_patch() {
        let buf = b"this is a test";
        let patch = generate_idempotent_patch(buf);

        let examples = [
            "",
            "this is a test",
            "1234",
            "\0"
        ];
        
        for example in examples.iter() {
            let mut new = Vec::new();
            let mut old = Cursor::new(example);

            apply_patch(&patch, &mut old, &mut new).unwrap();

            assert_eq!(&buf[..], &new[..]);
        }
    }

    #[test]
    fn test_simple_patch() {
        let buf = b"this is a test";
        let buf2 = b"this is really a cool test";
        let index = Index::compute(buf.to_vec());
        let patch = generate_full_patch(&index, &buf2[..]);
        
        let mut new = Vec::new();
        let mut old = Cursor::new(buf);

        apply_patch(&patch, &mut old, &mut new).unwrap();

        assert_eq!(&buf2[..], &new[..]);
    }

    #[test]
    fn test_full_patch() {
        let buf = b"this is a test 12345678 test";
        let buf2 = b"this is really a cool uftu 12345678 uftu";
        let index = Index::compute(buf.to_vec());
        let patch = generate_full_patch(&index, &buf2[..]);
        
        let mut new = Vec::new();
        let mut old = Cursor::new(buf);

        println!("done making patch");
        apply_patch(&patch, &mut old, &mut new).unwrap();

        assert_eq!(str::from_utf8(buf2).unwrap(), str::from_utf8(&new).unwrap());
    }
}
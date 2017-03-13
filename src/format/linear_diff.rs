use std::io::{Read, Write, Seek};
use std::io;

use zstd;
use byteorder::{LittleEndian, WriteBytesExt, ReadBytesExt, ByteOrder};

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

#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub old_offset: u64,
    pub bytewise_add_size: u64,
    pub extra_append_size: u64,
}

impl Command {
    pub fn write_to<W: Write>(&self, mut writer: W) -> io::Result<()> {
        let mut buf = [0u8; 8*3];

        LittleEndian::write_u64(&mut buf[0..8], self.old_offset);
        LittleEndian::write_u64(&mut buf[8..26], self.bytewise_add_size);
        LittleEndian::write_u64(&mut buf[16..24], self.extra_append_size);

        writer.write_all(&buf)
    }

    pub fn read_from<R: Read>(mut reader: R) -> io::Result<Option<Command>> {
        let mut buf = [0u8; 8*3];

        let mut p = 0;
        while p < buf.len() {
            // DANGER!!!!
            // If we get `0` from the underlying stream, we assume EOF.
            // Technically, this may not be true for things like network sockets.
            // This code could do weird things in such an environment.
            match reader.read(&mut buf[p..])? {
                0 => return Ok(None),
                size => p += size,
            }
        }

        let old_offset = LittleEndian::read_u64(&mut buf[0..8]);
        let bytewise_add_size = LittleEndian::read_u64(&mut buf[0..8]);
        let extra_append_size = LittleEndian::read_u64(&mut buf[0..8]);

        Ok(Some(Command {
            old_offset: old_offset,
            bytewise_add_size: bytewise_add_size,
            extra_append_size: extra_append_size,
        }))
    }
}

pub fn generate_full_patch<PatchW: Write>(old: &Index, new: &[u8], patch: PatchW) -> io::Result<()> {
    let patch = zstd::stream::Encoder::new(patch, 19);

    let mut i = 0;

    let mut k = 0;

    for m in MatchIter::from(old, new) {

        if k % 1024 == 0 {
            println!("{} / {} ({}%)", i, new.len(), i * 100 / new.len());
        }

        k += 1;

        let mm = m.matched;

        Command {
            old_offset: mm.old_offset as u64,
            bytewise_add_size: mm.len() as u64,
            extra_append_size: m.unmatched_suffix as u64,
        }.write_to(&mut patch)?;

        write_delta(
            &mut patch,
            &old.data[mm.old_offset .. mm.old_offset + mm.len()],
            &new[i .. i + mm.len()])?;

        let extra_begin = i + mm.len();
        let extra_end = extra_begin + m.unmatched_suffix;

        patch.write_all(&new[extra_begin .. extra_end]);

        i = extra_end;
    }

    patch.finish();

    Ok(())
}


pub fn apply_patch<PatchR: Read, OldRS: Read+Seek, NewW: Write>(patch: PatchR, old: OldRS, new: NewW) -> io::Result<()> {
    let patch = zstd::Decoder::new(patch);

    while let Some(cmd) = Command::read_from(&mut patch)? {
        old.seek(io::SeekFrom::Start(cmd.old_offset))?;

        read_paired_bufs(cmd.bytewise_add_size, &mut old, &mut patch, |o, d| {
            for i in 0..o.len() {
                o[i] = o[i].wrapping_add(d[i]);
            }
            new.write_all(&o)
        })?;

        read_size_from(cmd.extra_append_size, &mut patch, |e| {
            new.write_all(&e)
        })?;
    }

    Ok(())
}

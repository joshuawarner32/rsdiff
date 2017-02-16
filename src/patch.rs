use std::io::{self, Read, Write, Seek};
use std::fs::File;
use std::path::Path;
use std::cmp::{min, max, Ordering};

fn read_paired_bufs<F, R0: Read, R1: Read>(
    mut size: u64,
    mut r0: R0,
    mut r1: R1,
    mut f: F
) -> io::Result<()>
    where F: FnMut(&mut [u8], &mut [u8]) -> io::Result<()>
{
    let mut buf0 = [0u8; 1024];
    let mut buf1 = [0u8; 1024];

    let (mut p0, mut p1) = (0, 0);
    let mut base = 0;

    while size > 0 {
        let avail = min(buf0.len() as u64, size) as usize;
        if p0 < avail {
            let s0 = r0.read(&mut buf0[p0..avail])?;
            p0 += s0;
        }

        let avail = min(buf1.len() as u64, size) as usize;
        if p1 < avail {
            let s1 = r1.read(&mut buf1[p1..avail])?;
            p1 += s1;
        }

        let pmin = min(p0, p1);

        f(&mut buf0[base..pmin], &mut buf1[base..pmin])?;

        if p0 < p1 {
            for i in pmin..p1 {
                buf1[i - pmin] = buf1[i];
            }
        } else if p1 < p0 {
            for i in pmin..p0 {
                buf0[i - pmin] = buf0[i];
            }
        }

        base = 0;
        size -= pmin as u64;
    }

    Ok(())
}

fn read_size_from<F, R: Read>(mut size: u64, mut r: R, mut f: F) -> io::Result<()>
    where F: FnMut(&mut [u8]) -> io::Result<()>
{
    let mut buf = [0u8; 1024];

    let mut p = 0;
    let mut base = 0;

    while size > 0 {
        let avail = min(buf.len() as u64, size) as usize;
        if p < avail {
            let s = r.read(&mut buf[p..avail])?;
            p += s;
        }

        f(&mut buf[base..p])?;

        base = 0;
        size -= p as u64;
    }

    Ok(())
}

struct Command {
    bytewise_add_size: u64,
    extra_append_size: u64,
    oldfile_seek_offset: i64,
}

struct Patcher<CmdR, DiffR, ExtraR, OldRS, NewW> {
    cmd: CmdR,
    diff: DiffR,
    extra: ExtraR,
    old: OldRS,
    new: NewW,
}

fn offtin(buf: &[u8]) -> i64 {
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

impl<CmdR, DiffR, ExtraR, OldRS, NewW> Patcher<CmdR, DiffR, ExtraR, OldRS, NewW>
    where
        CmdR: Read,
        DiffR: Read,
        ExtraR: Read,
        OldRS: Read+Seek,
        NewW: Write
{
    fn read_command(&mut self) -> io::Result<Option<Command>> {
        let mut buf = [0u8; 8*3];

        let mut p = 0;
        loop {
            match self.cmd.read(&mut buf[p..]) {
                Ok(0) => break,
                Ok(size) => p += size,
                Err(e) => return Err(e)
            }
        }

        Ok(Some(Command {
            bytewise_add_size: offtin(&buf[0..8]) as u64,
            extra_append_size: offtin(&buf[8..16]) as u64,
            oldfile_seek_offset: offtin(&buf[16..24]),
        }))
    }

    fn apply(&mut self, c: &Command) -> io::Result<()> {
        self.append_delta(c.bytewise_add_size)?;
        self.append_extra(c.extra_append_size)?;
        self.seek_old(c.oldfile_seek_offset)?;
        Ok(())
    }

    fn append_delta(&mut self, size: u64) -> io::Result<()> {
        let new = &mut self.new;
        read_paired_bufs(size, &mut self.old, &mut self.diff, |o, d| {
            for i in 0..o.len() {
                o[i] += d[i];
            }
            new.write_all(&o)
        })
    }

    fn append_extra(&mut self, size: u64) -> io::Result<()> {
        let new = &mut self.new;
        read_size_from(size, &mut self.extra, |e| {
            new.write_all(&e)
        })
    }

    fn seek_old(&mut self, size: i64) -> io::Result<()> {
        self.old.seek(io::SeekFrom::Current(size)).map(|_|())
    }
}

/// `apply_raw` reads the three patch data channels (`cmd`, `diff`, `extra`),
/// and the `old` file stream (which must additionally be `Seek`), and writes
/// the new file to the `new` stream.
///
/// This allows trying out different compression algorithms and wrapper formats,
/// rather than accepting the defaults (bzip2, custom).
pub fn apply_raw<CmdR, DiffR, ExtraR, OldRS, NewW>(
    cmd: CmdR,
    diff: DiffR,
    extra: ExtraR,
    old: OldRS,
    new: NewW,
) -> io::Result<()>
    where
        CmdR: Read,
        DiffR: Read,
        ExtraR: Read,
        OldRS: Read+Seek,
        NewW: Write
{
    let mut p = Patcher {
        cmd: cmd,
        diff: diff,
        extra: extra,
        old: old,
        new: new,
    };

    while let Some(c) = p.read_command()? {
        p.apply(&c)?;
    }

    Ok(())
}

pub fn apply<OldRS, NewW>(diff: &[u8], old: OldRS, new: NewW) -> io::Result<()>
    where
        OldRS: Read+Seek,
        NewW: Write
{
    // TODO: read header, open streams, call apply_raw
    Ok(())
}
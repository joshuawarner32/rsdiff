use std::io::{self, Read, Write, Seek};
use std::fs::File;
use std::path::Path;
use std::cmp::{min, max, Ordering};

pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut contents = Vec::new();
    File::open(path)?.read_to_end(&mut contents)?;
    Ok(contents)
}

pub struct Index<'a>(Vec<&'a [u8]>);

#[derive(Debug)]
pub struct Diff {
    matches: usize,
    match_length_sum: usize
}

fn longest_prefix(a: &[u8], b: &[u8]) -> usize {
    let mut i = 0;
    let l = min(a.len(), b.len());
    while i < l {
        if a[i] != b[i] {
            break;
        }
        i += 1;
    }
    return i;
}

impl<'a> Index<'a> {
    pub fn new(data: &'a[u8]) -> Index<'a> {
        let mut index = Vec::new();

        for i in 0..data.len() {
            index.push(&data[i..]);
        }

        println!("Sorting");

        index.sort();

        println!("Done sorting");

        Index(index)
    }

    pub fn diff_to(&self, data: &'a [u8]) -> Diff {
        let mut i = 0;
        let mut matches = 0;
        let mut match_length_sum = 0;

        while i < data.len() {
            let d = &data[i..];
            let res = self.0.binary_search_by(|v| {
                let mut i = 0;
                let l = min(d.len(), v.len());
                while i < l {
                    if v[i] != d[i] {
                        return if v[i] < d[i] {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        };
                    }
                    i += 1;
                }
                Ordering::Equal
            });
            let len = match res {
                Ok(index) => data.len() - i,
                Err(index) => {
                    max(
                        if index < self.0.len() { longest_prefix(d, self.0[index]) } else { 0 },
                        if index > 0 { longest_prefix(d, self.0[index - 1]) } else { 0 })
                }
            };

            if len > 8 {
                matches += 1;
                match_length_sum += len;
            }


            i += max(1, len);
        }

        Diff {
            matches: matches,
            match_length_sum: match_length_sum,
        }
    }
}

struct Command {
    bytewise_add_size: usize,
    extra_append_size: usize,
    oldfile_seek_offset: i64,
}

struct Patcher<CmdR, DiffR, ExtraR, OldRS, NewW> {
    cmd: CmdR,
    diff: DiffR,
    extra: ExtraR,
    old: OldRS,
    new: NewW,
}

fn read_paired_bufs<F, R0: Read, R1: Read>(mut size: usize, mut r0: R0, mut r1: R1, mut f: F) -> io::Result<()>
    where F: FnMut(&mut [u8], &mut [u8]) -> io::Result<()>
{
    let mut buf0 = [0u8; 1024];
    let mut buf1 = [0u8; 1024];

    let (mut p0, mut p1) = (0, 0);
    let mut base = 0;

    while size > 0 {
        let avail = min(buf0.len(), size);
        if p0 < avail {
            let s0 = r0.read(&mut buf0[p0..avail])?;
            p0 += s0;
        }

        let avail = min(buf1.len(), size);
        if p1 < avail {
            let s1 = r1.read(&mut buf1[p1..avail])?;
            p1 += s1;
        }

        let pmin = min(p0, p1);

        f(&mut buf0[base..pmin], &mut buf1[base..pmin]);

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
        size -= pmin;
    }

    Ok(())
}

fn read_size_from<F, R: Read>(mut size: usize, mut r: R, mut f: F) -> io::Result<()>
    where F: FnMut(&mut [u8]) -> io::Result<()>
{
    let mut buf = [0u8; 1024];

    let mut p = 0;
    let mut base = 0;

    while size > 0 {
        let avail = min(buf.len(), size);
        if p < avail {
            let s = r.read(&mut buf[p..avail])?;
            p += s;
        }

        f(&mut buf[base..p]);

        base = 0;
        size -= p;
    }

    Ok(())
}

impl<CmdR, DiffR, ExtraR, OldRS, NewW> Patcher<CmdR, DiffR, ExtraR, OldRS, NewW>
    where
        CmdR: Read,
        DiffR: Read,
        ExtraR: Read,
        OldRS: Read+Seek,
        NewW: Write
{
    fn apply(&mut self, c: &Command) -> io::Result<()> {
        self.append_delta(c.bytewise_add_size)?;
        self.append_extra(c.extra_append_size)?;
        self.seek_old(c.oldfile_seek_offset)?;
        Ok(())
    }

    fn append_delta(&mut self, size: usize) -> io::Result<()> {
        let new = &mut self.new;
        read_paired_bufs(size, &mut self.old, &mut self.diff, |o, d| {
            for i in 0..o.len() {
                o[i] += d[i];
            }
            new.write_all(&o)
        })
    }

    fn append_extra(&mut self, size: usize) -> io::Result<()> {
        let new = &mut self.new;
        read_size_from(size, &mut self.extra, |e| {
            new.write_all(&e)
        })
    }

    fn seek_old(&mut self, size: i64) -> io::Result<()> {
        self.old.seek(io::SeekFrom::Current(size)).map(|_|())
    }
}

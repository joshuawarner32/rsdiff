use std::io::{self, Read, Write, BufReader};
use std::cmp::{min, max, Ordering};
use std::ops::Range;
use std::{mem, str};

use byteorder::{LittleEndian, WriteBytesExt, ReadBytesExt};
use bzip2;
use sha1::Sha1;

pub trait Cache {
    type Read: io::Read;
    type Write: io::Write;

    fn get(&self, digest: &[u8; 20]) -> io::Result<Option<Self::Read>>;
    fn get_writer(&self, digest: &[u8; 20]) -> io::Result<Self::Write>;
}

const VERSION: u8 = 5;

pub struct Index {
    pub data: Vec<u8>,
    offsets: Vec<usize>,
}

impl Index {
    pub fn from_cache_or_compute<C: Cache>(cache: C, data: Vec<u8>) -> io::Result<Index> {
        println!("Hashing");

        let mut sha1 = Sha1::new();
        sha1.update(&[VERSION]);
        sha1.update(&data);
        let digest = sha1.digest();
            

        if let Some(mut r) = cache.get(&digest.bytes())? {
            let mut offsets = Vec::new();

            let mut file_hash = [0u8; 20];
            r.read_exact(&mut file_hash)?;

            if file_hash == digest.bytes() {
                println!("Reading");

                // let mut r = BzDecoder::new(r);
                let mut r = BufReader::new(r);

                for _ in 0..data.len() as usize {
                    offsets.push(r.read_u64::<LittleEndian>()? as usize);
                }

                println!("Done");

                return Ok(Index {
                    data: data,
                    offsets: offsets,
                })
            }
        }

        let res = Index::compute(data);

        println!("Writing");

        res.serialize_to(&digest.bytes(), cache.get_writer(&digest.bytes())?)?;

        println!("Done");

        Ok(res)
    }

    pub fn compute(data: Vec<u8>) -> Index {
        println!("Initializing");
        let mut offsets = Vec::new();

        for i in 0..data.len() as usize {
            offsets.push(i);
        }

        println!("Sorting");

        offsets.sort_by(|a, b| {
            let sa = &data[*a as usize..];
            let sb = &data[*b as usize..];

            let mut i = 0;
            let l = min(sb.len(), sa.len());
            while i < l {
                if sa[i] != sb[i] {
                    return if sa[i] < sb[i] {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                }
                i += 1;
            }
            if sa.len() < sb.len() {
                Ordering::Less
            } else if sa.len() > sb.len() {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });

        Index {
            data: data,
            offsets: offsets,
        }
    }

    fn serialize_to<W: Write>(&self, digest: &[u8; 20], mut w: W) -> io::Result<()> {
        w.write_all(digest)?;

        for offset in &self.offsets {
            w.write_u64::<LittleEndian>(*offset as u64)?;
        }

        Ok(())
    }

    fn longest_match(&self, buf: &[u8]) -> Range<usize> {
        let res = self.offsets.binary_search_by(|&v| {
            let mut i = 0;
            let v = &self.data[v..];
            // println!("looking for {:?} in {:?} ",
            //    ::std::str::from_utf8(buf).unwrap(),
            //    ::std::str::from_utf8(v).unwrap());
            let l = min(buf.len(), v.len());
            while i < l {
                if v[i] != buf[i] {
                    return if v[i] < buf[i] {
                        // println!("returning Less");
                        Ordering::Less
                    } else {
                        // println!("returning Greater");
                        Ordering::Greater
                    };
                }
                i += 1;
            }
            if v.len() < buf.len() {
                // println!("returning Less");
                Ordering::Less
            } else if v.len() > buf.len() {
                // println!("returning Greater");
                Ordering::Greater
            } else {
                // println!("returning Equal");
                Ordering::Equal
            }
        });

        // println!("found [{}] at {:?}", unsafe { str::from_utf8_unchecked(buf) }, res);

        let (start, len) = match res {
            Ok(index) => {
                let start = self.offsets[index];
                let len = longest_prefix(buf, &self.data[start..]);
                (start, len)
            }
            Err(index) => {
                let lower_start = if index > 0 {
                    self.offsets[index - 1]
                } else {
                    self.data.len()
                };

                let upper_start = if index < self.offsets.len() {
                    self.offsets[index]
                } else {
                    self.data.len()
                };

                let lower_len = longest_prefix(buf, &self.data[lower_start..]);
                let upper_len = longest_prefix(buf, &self.data[upper_start..]);

                if lower_len > upper_len {
                    (lower_start, lower_len)
                } else {
                    (upper_start, upper_len)
                }
            }
        };

        start .. start + len
    }
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

#[derive(Debug)]
pub struct DiffStat {
    match_count: usize,
    match_length_sum: u64,
    partial_match_count: usize,
    partial_match_length_sum: u64,
}

fn partial_match_length(a: &[u8], b: &[u8]) -> usize {
    let mut cur_matches = 0;
    let mut last_good_i = 0;
    let mut i = 0;

    let len = min(a.len(), b.len());

    while (i - cur_matches < 8) && i < len {
        if cur_matches >= i / 2 {
            last_good_i = i;
        }

        if a[i] == b[i] {
            cur_matches += 1;
        }

        i += 1;
    }

    last_good_i
}

fn reverse_partial_match_length(a: &[u8], b: &[u8]) -> usize {
    let mut cur_matches = 0;
    let mut last_good_i = 0;
    let mut i = 0;

    let len = min(a.len(), b.len());

    while (i - cur_matches < 8) && i < len {
        if cur_matches >= i / 2 {
            last_good_i = i;
        }

        if a[len - i - 1] == b[len - i - 1] {
            cur_matches += 1;
        }

        i += 1;
    }

    last_good_i
}

impl DiffStat {
    pub fn from(old: &Index, new: &[u8]) -> DiffStat {
        let mut stat = DiffStat {
            match_count: 0,
            match_length_sum: 0,
            partial_match_count: 0,
            partial_match_length_sum: 0,
        };

        for m in MatchIter::from(old, new).map(|m| m.matched) {
            stat.match_count += 1;
            stat.match_length_sum += m.mid_exact_len as u64;

            stat.partial_match_length_sum += (m.upper_delta_len + m.lower_delta_len) as u64;
            if m.upper_delta_len + m.lower_delta_len > 0 {
                stat.partial_match_count += 1;
            }
        }

        stat
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Delta {
    pub old_offset: usize,
    pub lower_delta_len: usize,
    pub mid_exact_len: usize,
    pub upper_delta_len: usize,
}

impl Delta {
    pub fn lower_delta_range(&self) -> Range<usize> {
        self.old_offset .. self.old_offset + self.lower_delta_len
    }

    pub fn upper_delta_range(&self) -> Range<usize> {
        self.old_offset + self.lower_delta_len + self.mid_exact_len .. self.old_offset + self.len()
    }

    pub fn len(&self) -> usize {
        self.lower_delta_len + self.mid_exact_len + self.upper_delta_len
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Match {
    pub matched: Delta,
    pub unmatched_suffix: usize,
}

pub struct MatchIter<'a> {
    old: &'a Index,
    new: &'a [u8],
    i: usize,
    last_delta: Delta,
    last_end: usize,
}

impl<'a> MatchIter<'a> {
    pub fn from(old: &'a Index, new: &'a [u8]) -> MatchIter<'a> {
        MatchIter {
            old: old,
            new: new,
            i: 0,
            last_delta: Default::default(),
            last_end: 0,
        }
    }
}

impl<'a> Iterator for MatchIter<'a> {
    type Item = Match;
    
    fn next(&mut self) -> Option<Self::Item> {
        while self.i < self.new.len() {
            let m = self.old.longest_match(&self.new[self.i..]);

            // println!("i {} match {:?}", self.i, m);

            if m.len() >= 8 {
                let pml = partial_match_length(
                    &self.old.data[m.end..],
                    &self.new[self.i + m.len()..]);

                let rpml = reverse_partial_match_length(
                    &self.old.data[..m.start],
                    &self.new[self.last_end..self.i]);

                let begin = self.i - rpml;

                let last_end = self.last_end;
                self.last_end = self.i + m.len() + pml;

                self.i += max(1, m.len() + pml) as usize;

                let last_delta = mem::replace(&mut self.last_delta, Delta {
                    old_offset: m.start - rpml,
                    lower_delta_len: rpml,
                    mid_exact_len: m.len(),
                    upper_delta_len: pml,
                });

                if self.i > self.last_end || last_delta.len() > 0 {
                    return Some(Match {
                        matched: last_delta,
                        unmatched_suffix: begin - last_end,
                    });
                }
            } else {
                self.i += max(1, m.len()) as usize;
            }
        }

        self.i = min(self.i, self.new.len());

        if self.i > self.last_end || self.last_delta.len() > 0 {
            let suffix = self.i - self.last_end;
            self.last_end = self.i;
            return Some(Match {
                matched: mem::replace(&mut self.last_delta, Default::default()),
                unmatched_suffix: suffix,
            });
        }

        None
    }
}

pub fn write_zeros<W: Write>(mut w: W, count: u64) -> io::Result<()> {
    let buf = [0u8; 1024];
    let mut written = 0;
    while written < count {
        let s = w.write(&buf[..min(buf.len() as u64, (count - written)) as usize])?;
        written += s as u64;
        // println!("write zero {}", s);
    }
    Ok(())
}

pub fn write_delta<W: Write>(mut w: W, old: &[u8], new: &[u8]) -> io::Result<()> {
    assert_eq!(old.len(), new.len());
    let mut buf = [0u8; 1024];
    let mut written = 0;
    while written < old.len() {
        let to_write = min(buf.len(), old.len() - written);
        for i in  0..to_write {
            buf[i] = new[i + written].wrapping_sub(old[i + written]);
        }

        let s = w.write(&buf[..to_write as usize])?;
        // println!("write delta {}", s);
        written += s;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_simple_match() {
        let index = Index::compute(Vec::from(&b"this is a test"[..]));

        let matches = MatchIter::from(&index, b"this is a test").collect::<Vec<_>>();

        assert_eq!(matches, vec![
            Match {
                matched: Delta {
                    old_offset: 0,
                    lower_delta_len: 0,
                    mid_exact_len: 14,
                    upper_delta_len: 0,
                }, unmatched_suffix: 0
            }
        ]);
    }

    #[test]
    fn test_index_slightly_less_simple_match() {
        let index = Index::compute(Vec::from(&b"this is a test 12345678 test"[..]));

        println!("index:");
        for (i, &offset) in index.offsets.iter().enumerate() {
            println!("  {}:  {}: {:?}", i, offset, ::std::str::from_utf8(&index.data[offset..]).unwrap());
        }

        println!("");

        let matches = MatchIter::from(&index,
            b"this is really a cool uftu 12345678 uftu")
        .collect::<Vec<_>>();

        assert_eq!(matches, vec![
            Match {
                matched: Delta {
                    old_offset: 0,
                    lower_delta_len: 0,
                    mid_exact_len: 8,
                    upper_delta_len: 1
                },
                unmatched_suffix: 16
            },
            // NOTE: the following definitely seems sub-optimal.
            // We can probably do a better job here.
            Match {
                matched: Delta {
                    old_offset: 13,
                    lower_delta_len: 1,
                    mid_exact_len: 10,
                    upper_delta_len: 1
                },
                unmatched_suffix: 3
            }
        ]);
    }
}
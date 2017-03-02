use std::io::{self, Read, Write, BufReader};
use std::cmp::{min, max, Ordering};
use std::ops::Range;

use byteorder::{LittleEndian, WriteBytesExt, ReadBytesExt};
use bzip2::write::BzEncoder;
// use bzip2::read::BzDecoder;
use bzip2;
use sha1::Sha1;

use core::{Command, Header};

pub trait Cache {
    type Read: io::Read;
    type Write: io::Write;

    fn get(&self, digest: &[u8; 20]) -> io::Result<Option<Self::Read>>;
    fn get_writer(&self, digest: &[u8; 20]) -> io::Result<Self::Write>;
}

const VERSION: u8 = 5;

pub struct Index {
    data: Vec<u8>,
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

        // let mut w = BzEncoder::new(w, bzip2::Compression::Best);

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

        let start = self.offsets[match res {
            Ok(index) => index,
            Err(index) => {
                if index >= self.offsets.len() {
                    return 0..0;
                } else {
                    index
                }
            }
        }];

        let len = longest_prefix(buf, &self.data[start..]);

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
    pub fn from(from: &Index, to: &[u8]) -> DiffStat {
        let mut stat = DiffStat {
            match_count: 0,
            match_length_sum: 0,
            partial_match_count: 0,
            partial_match_length_sum: 0,
        };

        let mut i = 0;
        let mut k = 0;

        while i < to.len() {
            let m = from.longest_match(&to[i..]);

            // println!("longest match: {:?}", m);

            if k % 1000 == 0 {
                println!("{} / {} ({}%)", i, to.len(), i * 100 / to.len());
            }
            k += 1;

            let pml = if m.len() > 8 {
                let pml = partial_match_length(&from.data[m.end..], &to[i + m.len()..]);
                let rpml = reverse_partial_match_length(&from.data[..m.start], &to[..i]);
                // let m = m.start - rpml .. m.end + pml;

                stat.match_count += 1;
                stat.match_length_sum += m.len() as u64;

                stat.partial_match_length_sum += (pml + rpml) as u64;
                if pml + rpml > 0 {
                    stat.partial_match_count += 1;
                }

                pml
            } else {
                0
            };

            i += max(8, m.len() + pml) as usize;
        }

        stat
    }
}

fn write_zeros<W: Write>(mut w: W, count: u64) -> io::Result<()> {
    let buf = [0u8; 1024];
    let mut written = 0;
    while written < count {
        let s = w.write(&buf[..min(buf.len() as u64, (count - written)) as usize])?;
        written += s as u64;
    }
    Ok(())
}

pub fn generate_identity_patch(size: u64) -> Vec<u8> {
    let mut cmds = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    Command {
        bytewise_add_size: size,
        extra_append_size: 0,
        oldfile_seek_offset: 0,
    }.write_to(&mut cmds).unwrap();
    let cmds = cmds.finish().unwrap();

    let mut delta = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    write_zeros(&mut delta, size).unwrap();
    let delta = delta.finish().unwrap();

    let extra = BzEncoder::new(Vec::new(), bzip2::Compression::Best).finish().unwrap();

    let mut patch = Vec::new();

    Header {
        compressed_commands_size: cmds.len() as u64,
        compressed_delta_size: delta.len() as u64,
        new_file_size: size as u64,
    }.write_to(&mut patch).unwrap();

    patch.extend(&cmds);
    patch.extend(&delta);
    patch.extend(&extra);

    patch
}

pub fn generate_idempotent_patch(desired_output: &[u8]) -> Vec<u8> {
    let mut cmds = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    Command {
        bytewise_add_size: 0,
        extra_append_size: desired_output.len() as u64,
        oldfile_seek_offset: 0,
    }.write_to(&mut cmds).unwrap();
    let cmds = cmds.finish().unwrap();

    let delta = BzEncoder::new(Vec::new(), bzip2::Compression::Best).finish().unwrap();

    let mut extra = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    extra.write_all(&desired_output).unwrap();
    let extra = extra.finish().unwrap();

    let mut patch = Vec::new();

    Header {
        compressed_commands_size: cmds.len() as u64,
        compressed_delta_size: delta.len() as u64,
        new_file_size: desired_output.len() as u64,
    }.write_to(&mut patch).unwrap();

    patch.extend(&cmds);
    patch.extend(&delta);
    patch.extend(&extra);

    patch
}

pub fn generate_simple_patch(from: &Index, to: &[u8]) -> Vec<u8> {
    let mut i = 0;
    let mut k = 0;

    let mut cmds = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    let mut delta = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    let mut extra = BzEncoder::new(Vec::new(), bzip2::Compression::Best);

    let mut last_match = 0..0;
    let mut last_match_i = 0;

    while i < to.len() {
        let m = from.longest_match(&to[i..]); 

        // println!("longest match: {:?}", m);

        if k % 1000 == 0 {
            println!("{} / {} ({}%)", i, to.len(), i * 100 / to.len());
        }
        k += 1;

        let m_len = m.len();

        if m.len() > 8 {
            // Write out the previous command (now that we know the endpoint of the seek
            // and the extra size)
            Command {
                bytewise_add_size: last_match.len() as u64,
                extra_append_size: (i - last_match_i - last_match.len()) as u64,
                oldfile_seek_offset: (m.start as i64) - (last_match.end as i64),
            }.write_to(&mut cmds).unwrap();
            write_zeros(&mut delta, last_match.len() as u64).unwrap();
            extra.write_all(&to[last_match_i + last_match.len()..i]);

            last_match = m;
            last_match_i = i;
        }

        i += max(8, m_len + 1) as usize;
    }

    // Write out the last command
    Command {
        bytewise_add_size: last_match.len() as u64,
        extra_append_size: (to.len() - last_match_i - last_match.len()) as u64,
        oldfile_seek_offset: 0,
    }.write_to(&mut cmds).unwrap();
    write_zeros(&mut delta, last_match.len() as u64).unwrap();
    extra.write_all(&to[last_match_i + last_match.len()..]);


    let cmds = cmds.finish().unwrap();
    let delta = delta.finish().unwrap();
    let extra = extra.finish().unwrap();

    let mut patch = Vec::new();

    Header {
        compressed_commands_size: cmds.len() as u64,
        compressed_delta_size: delta.len() as u64,
        new_file_size: to.len() as u64,
    }.write_to(&mut patch).unwrap();

    patch.extend(&cmds);
    patch.extend(&delta);
    patch.extend(&extra);

    patch
}

pub fn generate_full_patch(from: &Index, to: &[u8]) -> Vec<u8> {
    let mut i = 0;
    let mut k = 0;

    let mut cmds = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    let mut delta = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    let mut extra = BzEncoder::new(Vec::new(), bzip2::Compression::Best);

    let mut last_match = 0..0;
    let mut last_match_i = 0;

    while i < to.len() {
        let m = from.longest_match(&to[i..]);

        // println!("longest match: {:?}", m);

        if k % 1000 == 0 {
            println!("{} / {} ({}%)", i, to.len(), i * 100 / to.len());
        }
        k += 1;

        let m_len = m.len();

        if m.len() > 8 {
            // Write out the previous command (now that we know the endpoint of the seek
            // and the extra size)
            Command {
                bytewise_add_size: last_match.len() as u64,
                extra_append_size: (i - last_match_i - last_match.len()) as u64,
                oldfile_seek_offset: (m.start as i64) - (last_match.end as i64),
            }.write_to(&mut cmds).unwrap();
            write_zeros(&mut delta, last_match.len() as u64).unwrap();
            extra.write_all(&to[last_match_i + last_match.len()..i]);

            last_match = m;
            last_match_i = i;
        }

        i += max(8, m_len + 1) as usize;
    }

    // Write out the last command
    Command {
        bytewise_add_size: last_match.len() as u64,
        extra_append_size: (to.len() - last_match_i - last_match.len()) as u64,
        oldfile_seek_offset: 0,
    }.write_to(&mut cmds).unwrap();
    write_zeros(&mut delta, last_match.len() as u64).unwrap();
    extra.write_all(&to[last_match_i + last_match.len()..]);


    let cmds = cmds.finish().unwrap();
    let delta = delta.finish().unwrap();
    let extra = extra.finish().unwrap();

    let mut patch = Vec::new();

    Header {
        compressed_commands_size: cmds.len() as u64,
        compressed_delta_size: delta.len() as u64,
        new_file_size: to.len() as u64,
    }.write_to(&mut patch).unwrap();

    patch.extend(&cmds);
    patch.extend(&delta);
    patch.extend(&extra);

    patch
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_longest_match() {
        let index = Index::compute(Vec::from(&b"this is a test"[..]));

        println!("index:");
        for &offset in &index.offsets {
            println!("  {}: {:?}", offset, ::std::str::from_utf8(&index.data[offset..]).unwrap());
        }

        let stat = DiffStat::from(&index, b"this is a test");
        assert_eq!(stat.match_count, 1);
        assert_eq!(stat.match_length_sum, b"this is a test".len() as u64);
    }
}
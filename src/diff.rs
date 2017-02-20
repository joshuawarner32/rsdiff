use std::io::{self, Read, Write, BufReader};
use std::cmp::{min, Ordering};

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

const VERSION: u8 = 4;

pub struct Index {
    data_sha1: [u8; 20],
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
            
        let mut offsets = Vec::new();

        if let Some(mut r) = cache.get(&digest.bytes())? {

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
                    data_sha1: digest.bytes(),
                    data: data,
                    offsets: offsets,
                })
            }
        }


        println!("Initializing");

        for i in 0..data.len() as usize {
            offsets.push(i);
        }

        println!("Sorting");

        offsets.sort_by(|a, b| {
            let sa = &data[*a as usize..];
            let sb = &data[*b as usize..];
            sa.cmp(sb)
        });

        let res = Index {
            data_sha1: digest.bytes(),
            data: data,
            offsets: offsets,
        };

        println!("Writing");

        res.serialize_to(cache.get_writer(&digest.bytes())?)?;

        println!("Done");

        Ok(res)
    }

    fn serialize_to<W: Write>(&self, mut w: W) -> io::Result<()> {
        w.write_all(&self.data_sha1)?;

        // let mut w = BzEncoder::new(w, bzip2::Compression::Best);

        for offset in &self.offsets {
            w.write_u64::<LittleEndian>(*offset as u64)?;
        }

        Ok(())
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
    match_length_sum: u64
}

impl DiffStat {
    pub fn from(from: &Index, to: &Index) -> DiffStat {
        let mut match_count = 0;
        let mut match_length_sum = 0;

        let mut it_a = from.offsets.iter().map(|&o| &from.data[o..]).fuse();
        let mut it_b = to.offsets.iter().map(|&o| &to.data[o..]).fuse();

        let mut i = 0;

        if let (Some(mut a), Some(mut b)) = (it_a.next(), it_b.next()) {
            loop {
                println!("{}", i);
                i += 1;
                let prefix_len = longest_prefix(a, b);

                if prefix_len > 8 {
                    match_count += 1;
                    match_length_sum += prefix_len as u64;
                }

                let (new_a, new_b) = match a.cmp(b) {
                    Ordering::Less => (it_a.next(), Some(b)),
                    Ordering::Greater => (Some(a), it_b.next()),
                    Ordering::Equal => (it_a.next(), it_b.next()),
                };

                if !(new_a.is_some() || new_b.is_some()) {
                    break;
                }

                a = new_a.unwrap_or(a);
                b = new_b.unwrap_or(b);
            }
        }

        DiffStat {
            match_count: match_count,
            match_length_sum: match_length_sum,
        }
    }
}

pub fn generate_identity_patch(size: u64) -> Vec<u8> {
    let mut cmds = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    Command {
        bytewise_add_size: size,
        extra_append_size: 0,
        oldfile_seek_offset: 0,
    }.write_to(&mut cmds).unwrap();
    let cmds = cmds.finish().unwrap();

    let mut diff = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    let buf = [0u8; 1024];
    let mut written = 0;
    while written < size {
        let s = diff.write(&buf[..min(buf.len() as u64, (size - written)) as usize]).unwrap();
        written += s as u64;
    }
    let diff = diff.finish().unwrap();

    let extra = BzEncoder::new(Vec::new(), bzip2::Compression::Best).finish().unwrap();

    let mut patch = Vec::new();

    Header {
        compressed_commands_size: cmds.len() as u64,
        compressed_delta_size: diff.len() as u64,
        new_file_size: size as u64,
    }.write_to(&mut patch).unwrap();

    patch.extend(&cmds);
    patch.extend(&diff);
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

    let diff = BzEncoder::new(Vec::new(), bzip2::Compression::Best).finish().unwrap();

    let mut extra = BzEncoder::new(Vec::new(), bzip2::Compression::Best);
    extra.write_all(&desired_output).unwrap();
    let extra = extra.finish().unwrap();

    let mut patch = Vec::new();

    Header {
        compressed_commands_size: cmds.len() as u64,
        compressed_delta_size: diff.len() as u64,
        new_file_size: desired_output.len() as u64,
    }.write_to(&mut patch).unwrap();

    patch.extend(&cmds);
    patch.extend(&diff);
    patch.extend(&extra);

    patch
}
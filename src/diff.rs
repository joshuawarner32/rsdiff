use std::io::{self, Read, Write, Seek};
use std::fs::File;
use std::path::Path;
use std::cmp::{min, max, Ordering};

use byteorder::{LittleEndian, WriteBytesExt};
use bzip2::write::BzEncoder;
use bzip2;
use sha1::{Sha1, Digest};

use index::Index;

#[derive(Debug)]
pub struct DiffStat {
    matches: usize,
    match_length_sum: u64
}

impl DiffStat {
    pub fn from<I: Index>(index: I, new_data: &[u8]) -> DiffStat {
        let mut i = 0;
        let mut matches = 0;
        let mut match_length_sum = 0;

        while i < new_data.len() {
            let d = &new_data[i..];

            let range = index.find_longest_prefix(d);

            let len = (range.end - range.start) as usize;

            if len > 8 {
                matches += 1;
                match_length_sum += len as u64;
            }

            i += max(1, len);

            println!("{} / {} ({}%)", i, new_data.len(), i * 100 / new_data.len());
        }

        DiffStat {
            matches: matches,
            match_length_sum: match_length_sum,
        }
    }
}
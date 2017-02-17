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
    pub fn from<FromI: Index, ToI: Index>(from: FromI, to: ToI) -> DiffStat {
        let mut matches = 0;
        let mut match_length_sum = 0;

        let mut it_a = from.sorted_ranges();
        let mut it_b = to.sorted_ranges();

        let mut a = it_a.next();
        let mut b = it_b.next();

        loop {
            match (a, b) {
                (Some(a), Some(b)) => {

                }
                _ => {
                    break;
                }
            }
        }

        DiffStat {
            matches: matches,
            match_length_sum: match_length_sum,
        }
    }
}
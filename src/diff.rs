use std::io::{self, Read, Write, Seek};
use std::fs::File;
use std::path::Path;
use std::cmp::{min, max, Ordering};

pub trait Index {
    fn find_longest_prefix(&self, buffer: &[u8]) -> u64;
}

pub struct SuffixArray<'a>(Vec<&'a [u8]>);

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

impl<'a> SuffixArray<'a> {
    pub fn new(data: &'a[u8]) -> SuffixArray<'a> {
        let mut array = Vec::new();

        for i in 0..data.len() {
            array.push(&data[i..]);
        }

        println!("Sorting");

        array.sort();

        println!("Done sorting");

        SuffixArray(array)
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

use std::io::{self, Read, Write, Seek, Cursor};
use std::cmp::min;

use bzip2::bufread::BzDecoder;

use format::bsdiff::{
    Command,
    CommandReader,
    Header,
};

pub fn read_paired_bufs<F, R0: Read, R1: Read>(
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
        println!("base {} size {}", base, size);
        let avail = min(buf0.len() as u64, size) as usize;
        println!("avail {} p0 {}", avail, p0);
        if p0 < avail {
            let s0 = r0.read(&mut buf0[p0..avail])?;
            p0 += s0;
            if s0 == 0 {
                println!("got s0 0");
                break;
            }
            println!("s0 {} p0 {}", s0, p0);
        }

        let avail = min(buf1.len() as u64, size) as usize;
        println!("avail {} p1 {}", avail, p1);
        if p1 < avail {
            let s1 = r1.read(&mut buf1[p1..avail])?;
            p1 += s1;
            if s1 == 0 {
                println!("got s1 0");
                break;
            }
            println!("s1 {} p1 {}", s1, p1);
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

        p0 -= pmin;
        p1 -= pmin;

        let processed = pmin - base;

        println!("size {} processed {}", size, processed);

        size -= processed as u64;
        base = 0;
    }

    Ok(())
}

pub fn read_size_from<F, R: Read>(mut size: u64, mut r: R, mut f: F) -> io::Result<()>
    where F: FnMut(&mut [u8]) -> io::Result<()>
{
    let mut buf = [0u8; 1024];

    let mut p = 0;
    let mut base = 0;

    while size > 0 {
        let avail = min(buf.len() as u64, size) as usize;
        if p < avail {
            let s = r.read(&mut buf[p..avail])?;
            assert!(s <= avail - p);
            p += s;
        }

        if p == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "failed to fill whole buffer"));
        }

        f(&mut buf[base..p])?;

        base = 0;
        size -= p as u64;
        p = 0;
    }

    Ok(())
}

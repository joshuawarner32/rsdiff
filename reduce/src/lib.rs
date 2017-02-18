extern crate rand;

use rand::Rng;

pub fn reduce_step<R: Rng, F: Fn(&[u8]) -> bool>(rng: &mut R, data: &[u8], f: F, limit: usize) -> Option<Vec<u8>> {

    if data.len() == 0 {
        return None;
    }

    let mut trial0 = Vec::with_capacity(data.len());
    let mut trial1 = Vec::with_capacity(data.len());

    for b in data {
        trial0.push(*b);
    }

    let mut i = 0;

    loop {
        let a = rng.gen_range(0, trial0.len() + 1);
        let b = rng.gen_range(0, trial0.len() + 1);

        if a == b {
            continue;
        }

        let begin = ::std::cmp::min(a, b);
        let end = ::std::cmp::max(a, b);

        trial1.clear();

        if rng.gen() {
            trial1.extend_from_slice(&trial0[..begin]);
            trial1.extend_from_slice(&trial0[end..]);
        } else {
            trial1.extend_from_slice(&trial0);

            let mut all_zero = true;
            for i in begin..end {
                if trial1[i] != 0 {
                    all_zero = false;
                    trial1[i] = rng.gen_range(0, trial0[i]);
                }
            }

            if all_zero {
                i += 1;

                if i >= limit {
                    return None;
                }

                continue;
            }
        }


        if f(&trial1) {
            return Some(trial1);
        } else {
            i += 1;

            if i >= limit {
                return None;
            }
        }
    }
}

pub fn reduce<R: Rng, T: PartialEq, F: Fn(&[u8]) -> T>(rng: &mut R, data: &[u8], f: F, limit: usize) -> Vec<u8> {
    let reference = f(data);

    let mut trial0 = Vec::with_capacity(data.len());
    trial0.extend_from_slice(&data);

    let check = |data: &[u8]| {
        reference == f(data)
    };

    loop {
        match reduce_step(rng, &trial0, &check, limit) {
            None => break,
            Some(x) => trial0 = x
        };
    }

    trial0
}

pub fn reduce_each<R: Rng, T: PartialEq, F: Fn(&[Vec<u8>]) -> T>(rng: &mut R, data: &[Vec<u8>], f: F, limit: usize) -> Vec<Vec<u8>> {
    let reference = f(data);

    let mut trial0 = data.iter().map(|e|{e.clone()}).collect::<Vec<_>>();

    loop {
        let mut updated = 0;
        for i in 0..trial0.len() {
            let res = {
                let check = |data: &[u8]| {
                    let full = trial0.iter().enumerate()
                        .map(|(j, e)|{ if j == i { data.to_vec() } else { e.clone() } })
                        .collect::<Vec<_>>();
                    reference == f(&full)
                };
                reduce_step(rng, &trial0[i], &check, limit)
            };

            match res {
                None => {},
                Some(x) => {
                    trial0[i] = x;
                    updated += 1;
                }
            };
        }

        if updated == 0 {
            break;
        }
    }

    trial0
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_reduce() {
        assert_eq!(
            reduce(
                &mut thread_rng(),
                b"this is a test",
                |data| { data.iter().any(|b| {*b == b'a' as u8}) },
                20),
            b"a");
    }

    #[test]
    fn test_reduce_each() {
        assert_eq!(
            reduce_each(
                &mut thread_rng(),
                &[b"this is a test".to_vec(), b"this is another test".to_vec()],
                |data| {
                    data[0].iter().any(|b| {*b == b'a' as u8}) &&
                    data[1].iter().any(|b| {*b == b'n' as u8})
                },
                20),
            vec!(b"a".to_vec(), b"n".to_vec()));
    }
}
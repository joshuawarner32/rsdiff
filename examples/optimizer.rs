extern crate bsdiff;

fn main() {
    let a = bsdiff::load("tests/avian_linux").unwrap();
    let b = bsdiff::load("tests/avian_pr_linux").unwrap();

    let index = bsdiff::Index::new(&a);

    let diff = index.diff_to(&b);

    println!("diff {:?}", diff);
}
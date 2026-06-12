use std::fs;

use crate::dss::dss_file::DssFile;

mod dss;

fn main() {
    let save1 = fs::read("./data011Slot.bin").expect("Introuvable");
    let save2 = fs::read("./data012Slot.bin").expect("Introuvable");

    let save_file = DssFile::new(save1);
    // let save_file2 = DssFile::new(save2);

    println!("{}", save_file);
    // println!("{}", save_file2);
}

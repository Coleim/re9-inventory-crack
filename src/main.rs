use std::fs;

use crate::{
    dsss::file::File,
    mandarin::{cipher, crack_steamid::crack_steamid},
    rsz::reader::read,
};

mod dsss;
mod mandarin;
mod rsz;

pub const HEADER_LEN: usize = 16;
pub fn payload(file: &[u8]) -> &[u8] {
    let len = file.len();
    &file[HEADER_LEN..len - 4]
}

pub fn decrypted_len(file: &[u8]) -> u64 {
    let len = file.len();
    u64::from_le_bytes(file[len - 12..len - 4].try_into().unwrap())
}

fn main() {
    let save1 = fs::read("./data011Slot.bin").expect("Introuvable");
    // let save1 = fs::read("./test001_with_munitions.bin").expect("Introuvable");
    let save2 = fs::read("./test_001_after_reload.bin").expect("Introuvable");

    let save_file = File::new(save1);
    let save_file2 = File::new(save2);
    // println!("Starting crack based on {}", payload);

    println!("Decrypted data len: {} ", save_file.decrypted_data_len);
    match crack_steamid(
        save_file.first_ciphertext_bytes(),
        save_file.decrypted_data_len,
    ) {
        Some(id) => {
            println!("SteamID64: {id}");

            let decrypted = cipher::decrypt(&save_file.payload, id, save_file.decrypted_data_len);
            // fs::write("output.bin", &result).expect("Failed to write file");

            let mut tree = read(&decrypted); // Vec<Node>, plusieurs roots
            println!(" Vec Root {}", tree.len());
        }
        None => eprintln!("no SteamID found"),
    }

    // println!("{}", save_file);
    // println!("{}", save_file2);
    //
    //
}

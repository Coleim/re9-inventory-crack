use crate::mandarin::cipher;
use crate::murmur3;

pub mod file;
pub mod header;

pub const MAGIC: &[u8; 4] = b"DSSS";
pub const VERSION: u32 = 2;
pub const FLAG_MANDARIN: u32 = 0x10;
pub const HEADER_LEN: usize = 16;
const MURMUR_LEN: usize = 4;
const DECRYPTED_LEN_SIZE: usize = 8;

/// MurmurHash3_x86_32 of the whole file, excluding the trailing 4-byte
/// checksum itself, seeded with `0xffffffff` (matches the game's format).
pub fn murmur(data: &[u8]) -> u32 {
    murmur3::hash32(data, 0xffffffff)
}

pub fn payload(file: &[u8]) -> &[u8] {
    let len = file.len();
    &file[HEADER_LEN..len - MURMUR_LEN]
}

pub fn decrypted_len(file: &[u8]) -> u64 {
    let len = file.len();
    u64::from_le_bytes(file[len - MURMUR_LEN - DECRYPTED_LEN_SIZE..len - MURMUR_LEN].try_into().unwrap())
}

pub fn verify_file_hash(file: &[u8]) -> bool {
    let len = file.len();
    let stored = u32::from_le_bytes(file[len - MURMUR_LEN..].try_into().unwrap());
    murmur(&file[..len - MURMUR_LEN]) == stored
}

/// Decrypt a full `.bin` save file (header + encrypted payload + trailer)
/// into its raw RSZ plaintext, given the file's SteamID.
pub fn decrypt(file: &[u8], steamid: u64) -> Vec<u8> {
    cipher::decrypt(payload(file), steamid, decrypted_len(file))
}

/// Re-encrypt a decrypted RSZ payload into a full, loadable DSSS save file:
/// `[header(16)][encrypted payload][decrypted_len: u64][padding to 4][murmur3 checksum: u32]`.
pub fn build(plain: &[u8], steamid: u64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&FLAG_MANDARIN.to_le_bytes());
    out.resize(HEADER_LEN, 0);

    let encrypted = cipher::encrypt(plain, steamid);
    out.extend_from_slice(&encrypted);
    out.extend_from_slice(&(plain.len() as u64).to_le_bytes());

    while out.len() % 4 != 0 {
        out.push(0);
    }
    let hash = murmur(&out);
    out.extend_from_slice(&hash.to_le_bytes());
    out
}

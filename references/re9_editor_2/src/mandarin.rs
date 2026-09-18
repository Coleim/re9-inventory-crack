use crate::bignum::{mod_exp, to_bytes_le, to_int};
use crate::elgamal::{self, ElGamal};
use crate::splitmix;
use aes::Aes128;
use cipher::{KeyIvInit, StreamCipher};
use hex_literal::hex;
use num_bigint::BigInt;
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

type Aes128Ofb = ofb::Ofb<Aes128>;

pub const SEED_RSA: u64 = 0;
pub const SEED_ENC: u64 = 0x61f6868699c14dfa;

pub const RSA_N: [u8; 128] = hex!(
    "4fa448364f5b3507e945075cc21994bdedef96962c74d53159d50a5c62ed5086"
    "4885ddfe79705dfad0b638220ca2299fccae152164590cc89d33698452a8f641"
    "6107a6952f126bb21ee3e332d2285db728c09bfa8cbd4c3b13b358b9838dea7c"
    "f39dc12e37066a09cf7809a0d0ea06c3bbaa14776400f403f863ed83b5bdd3c2"
);

const META: usize = 0x210;
const UNIT: usize = 0x4000;

pub const STEAMID_BASE: u64 = 0x0110_0001_0000_0000;
pub const STEAMID_COUNT: u64 = 1u64 << 32;

fn block_plan(decrypted_len: u64) -> (Vec<u8>, usize, u64) {
    let num_potential = ((decrypted_len & 0x3fff != 0) as u64) + (decrypted_len >> 0xe);
    let mut sizes = vec![0u8; num_potential as usize];
    let mut state = SEED_ENC;
    for i in 0..num_potential as usize {
        sizes[i] = (state & 7) as u8 + 1;
        state = splitmix::next(state);
    }
    let mut leftover = decrypted_len;
    let mut real = 0usize;
    for i in 0..num_potential as usize {
        let span = sizes[i] as u64 * UNIT as u64;
        real += 1;
        if leftover <= span {
            break;
        }
        leftover -= span;
    }
    (sizes, real, state)
}

pub fn decrypt(encrypted: &[u8], decrypted_len: u64, key: u64) -> Result<Vec<u8>, String> {
    let (sizes, real, mut state) = block_plan(decrypted_len);
    state = state.wrapping_add(!key);
    let auth = ElGamal::new(!key);

    let mut out = vec![0u8; decrypted_len as usize];
    let mut enc_off = 0usize;
    let mut dec_off = 0usize;
    let mut remaining = decrypted_len as usize;

    for i in 0..real {
        let block_size = sizes[i] as usize * UNIT;
        let read_size = block_size + META;
        if enc_off + read_size > encrypted.len() {
            return Err(format!("block {i}: out of bounds"));
        }
        let mut buf = encrypted[enc_off..enc_off + read_size].to_vec();

        let mut k = [0u8; 16];
        let mut iv = [0u8; 16];
        for j in 0..16 {
            state = splitmix::next(state);
            k[j] = state as u8;
            iv[j] = (state >> 8) as u8;
        }
        for j in 0..META {
            state = splitmix::next(state);
            buf[j] ^= state as u8;
        }

        let mut key_iv_check = [0u8; 32];
        for c in 0..4 {
            let mut c0 = [0u8; 64];
            let mut c1 = [0u8; 64];
            c0.copy_from_slice(&buf[c * 128..c * 128 + 64]);
            c1.copy_from_slice(&buf[c * 128 + 64..c * 128 + 128]);
            let word = auth.decrypt_word(&c0, &c1);
            key_iv_check[c * 8..c * 8 + 8].copy_from_slice(&word);
        }
        let key_ok = key_iv_check[0..16] == k && key_iv_check[16..32] == iv;
        if !key_ok {
            return Err(format!("block {i}: ElGamal key/iv auth mismatch"));
        }

        let target_checksum = u64::from_le_bytes(buf[0x200..0x208].try_into().unwrap());

        let mut cipher = Aes128Ofb::new(&k.into(), &iv.into());
        cipher.apply_keystream(&mut buf[META..META + block_size]);

        let to_copy = block_size.min(remaining);
        let checksum = cityhasher::hash::<u64>(&buf[META..META + to_copy]);
        if checksum != target_checksum {
            return Err(format!(
                "block {i}: checksum mismatch (key_check={key_ok}) target={target_checksum:#x} got={checksum:#x}"
            ));
        }

        out[dec_off..dec_off + to_copy].copy_from_slice(&buf[META..META + to_copy]);
        remaining = remaining.wrapping_sub(block_size);
        dec_off += to_copy;
        enc_off += read_size;
    }
    Ok(out)
}

pub fn encrypt(data: &[u8], key: u64) -> Vec<u8> {
    let mut state_a = SEED_RSA;
    let mut rands = [0u8; 0x20];
    for i in 0..32 {
        splitmix::step(&mut state_a);
        rands[i] = state_a as u8;
    }
    rands[0..8].copy_from_slice(&(!key).to_le_bytes());
    let n = to_int(&RSA_N);
    let rands_int = to_int(&rands[0..32]);
    let e = BigInt::from(65537u64);
    let encrypted_key = mod_exp(&rands_int, &e, &n);

    let data_len = data.len() as u64;
    let (sizes, real, mut state) = block_plan(data_len);
    state = state.wrapping_add(!key);
    let auth = ElGamal::new(!key);

    let mut total = 0x80usize;
    for i in 0..real {
        total += sizes[i] as usize * UNIT + META;
    }
    let mut out = vec![0u8; total];
    let mut enc_off = 0usize;
    let mut dec_off = 0usize;
    let mut remaining = data_len as usize;

    for i in 0..real {
        let block_size = sizes[i] as usize * UNIT;
        let to_copy = block_size.min(remaining);
        let mut buf = vec![0u8; block_size + META];

        let mut k = [0u8; 16];
        let mut iv = [0u8; 16];
        for j in 0..16 {
            state = splitmix::next(state);
            k[j] = state as u8;
            iv[j] = (state >> 8) as u8;
        }

        buf[META..META + to_copy].copy_from_slice(&data[dec_off..dec_off + to_copy]);
        let checksum = cityhasher::hash::<u64>(&buf[META..META + to_copy]);

        let mut cipher = Aes128Ofb::new(&k.into(), &iv.into());
        cipher.apply_keystream(&mut buf[META..META + block_size]);

        let mut key_iv = [0u8; 32];
        key_iv[0..16].copy_from_slice(&k);
        key_iv[16..32].copy_from_slice(&iv);
        for c in 0..4 {
            let mut word = [0u8; 8];
            word.copy_from_slice(&key_iv[c * 8..c * 8 + 8]);
            let (c0, c1) = auth.encrypt_word(word);
            buf[c * 128..c * 128 + 64].copy_from_slice(&c0);
            buf[c * 128 + 64..c * 128 + 128].copy_from_slice(&c1);
        }
        buf[0x200..0x208].copy_from_slice(&checksum.to_le_bytes());

        for j in 0..META {
            state = splitmix::next(state);
            buf[j] ^= state as u8;
        }

        out[enc_off..enc_off + block_size + META].copy_from_slice(&buf);
        remaining = remaining.wrapping_sub(to_copy);
        dec_off += to_copy;
        enc_off += block_size + META;
    }

    let integer = to_bytes_le::<0x80>(&encrypted_key);
    out[enc_off..enc_off + 0x80].copy_from_slice(&integer);
    out.truncate(enc_off + 0x80);
    out
}

pub fn crack(encrypted: &[u8], decrypted_len: u64, base: u64, count: u64) -> Option<u64> {
    crack_with(encrypted, decrypted_len, base, count, |_| {})
}

pub fn crack_with<F: Fn(u64) + Sync>(
    encrypted: &[u8],
    decrypted_len: u64,
    base: u64,
    count: u64,
    report: F,
) -> Option<u64> {
    let (_, _, initial_state) = block_plan(decrypted_len);
    let x0 = elgamal::constant_x0_low8();
    let mut target = [0u8; 8];
    for i in 0..8 {
        target[i] = encrypted[i] ^ x0[i];
    }

    let progress = AtomicU64::new(0);
    let chunk = 1_000_000u64;
    let num_chunks = count.div_ceil(chunk);

    (0..num_chunks).into_par_iter().find_map_any(|ci| {
        let start = ci * chunk;
        let end = (start + chunk).min(count);
        let mut found = None;
        for off in start..end {
            let steamid = base.wrapping_add(off);
            let mut s = initial_state.wrapping_add(!steamid);
            for _ in 0..16 {
                s = splitmix::next(s);
            }
            let mut mask = [0u8; 8];
            for i in 0..8 {
                s = splitmix::next(s);
                mask[i] = s as u8;
            }
            if mask == target {
                found = Some(steamid);
                break;
            }
        }
        let done = progress.fetch_add(end - start, Ordering::Relaxed) + (end - start);
        report(done);
        found
    })
}

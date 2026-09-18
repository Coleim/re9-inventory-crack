use crate::bignum::{mod_exp, to_bytes_le, to_int};
use hex_literal::hex;
use num_bigint::BigInt;

pub const P: [u8; 32] =
    hex!("f33b6fb972a0b72515e45c391829e182ad8a9bdc0a64d3444d79c810ab863717");
pub const Q: [u8; 32] =
    hex!("f99db75c39d0db920a72ae1c8c9470c156c54d6e05b269a2a63c648855c39b0b");
pub const R: [u8; 32] =
    hex!("e66f544afcce68c5ef07b9a07b277585344a1db61376e831f73b9fbd5f44f715");

pub struct ElGamal {
    p: BigInt,
    r: BigInt,
    s: BigInt,
    u: BigInt,
    e: BigInt,
}

impl ElGamal {
    pub fn new(u: u64) -> Self {
        let p = to_int(&P);
        let q = to_int(&Q);
        let r = to_int(&R);
        let u = BigInt::from(u) % &q;
        let s = mod_exp(&r, &u, &p);
        let e = BigInt::from(0x14u64);
        Self { p, r, s, u, e }
    }

    pub fn encrypt_word(&self, word: [u8; 8]) -> ([u8; 64], [u8; 64]) {
        let x0 = mod_exp(&self.r, &self.e, &self.p);
        let x1 = mod_exp(&self.s, &self.e, &self.p);
        let pt = to_int(&word);
        let ct = x1 * pt;
        (to_bytes_le::<64>(&x0), to_bytes_le::<64>(&ct))
    }

    pub fn decrypt_word(&self, c0: &[u8; 64], c1: &[u8; 64]) -> [u8; 8] {
        let x0 = to_int(c0);
        let ct = to_int(c1);
        let x = mod_exp(&x0, &self.u, &self.p);
        let k = ct / x;
        to_bytes_le::<8>(&k)
    }
}

pub fn constant_x0_low8() -> [u8; 8] {
    let p = to_int(&P);
    let r = to_int(&R);
    let e = BigInt::from(0x14u64);
    let x0 = mod_exp(&r, &e, &p);
    let full = to_bytes_le::<64>(&x0);
    let mut out = [0u8; 8];
    out.copy_from_slice(&full[0..8]);
    out
}

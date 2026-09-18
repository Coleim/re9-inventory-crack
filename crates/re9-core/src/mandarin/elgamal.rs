use hex_literal::hex;
use num_bigint::{BigInt, Sign};

const P: [u8; 32] = hex!("f33b6fb972a0b72515e45c391829e182ad8a9bdc0a64d3444d79c810ab863717");
const Q: [u8; 32] = hex!("f99db75c39d0db920a72ae1c8c9470c156c54d6e05b269a2a63c648855c39b0b");
const R: [u8; 32] = hex!("e66f544afcce68c5ef07b9a07b277585344a1db61376e831f73b9fbd5f44f715");

fn to_int(bytes: &[u8]) -> BigInt {
    BigInt::from_bytes_le(Sign::Plus, bytes)
}

fn to_bytes_le<const N: usize>(n: &BigInt) -> [u8; N] {
    let mut out = [0u8; N];
    let digits = n.to_bytes_le().1;
    let len = digits.len().min(N);
    out[..len].copy_from_slice(&digits[..len]);
    out
}

fn mod_exp(base: &BigInt, exp: &BigInt, modulus: &BigInt) -> BigInt {
    base.modpow(exp, modulus)
}

/// ElGamal encryption used to embed a block's AES key/IV in the
/// "keyFragmentsBank". The ephemeral exponent is fixed (`e = 0x14`), which
/// is what allows the SteamID to be recovered via `crack_steamid`: `c0` is
/// always the same constant (`x0`) no matter the SteamID or message.
pub struct ElGamal {
    p: BigInt,
    r: BigInt,
    s: BigInt,
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
        Self { p, r, s, e }
    }

    pub fn encrypt_word(&self, word: [u8; 8]) -> ([u8; 64], [u8; 64]) {
        let x0 = mod_exp(&self.r, &self.e, &self.p);
        let x1 = mod_exp(&self.s, &self.e, &self.p);
        let pt = to_int(&word);
        let ct = x1 * pt;
        (to_bytes_le::<64>(&x0), to_bytes_le::<64>(&ct))
    }
}

pub fn compute_x0() -> [u8; 8] {
    let e = BigInt::from(0x14u64);
    let p = to_int(&P);
    let r = to_int(&R);
    let x0 = mod_exp(&r, &e, &p);
    to_bytes_le::<8>(&x0)
}

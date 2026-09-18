use num_bigint::{BigInt, Sign};

pub fn to_int(bytes: &[u8]) -> BigInt {
    BigInt::from_bytes_le(Sign::Plus, bytes)
}

pub fn to_bytes_le<const N: usize>(n: &BigInt) -> [u8; N] {
    let mut out = [0u8; N];
    let digits = n.to_bytes_le().1;
    let len = digits.len().min(N);
    out[..len].copy_from_slice(&digits[..len]);
    out
}

pub fn mod_exp(base: &BigInt, exp: &BigInt, modulus: &BigInt) -> BigInt {
    base.modpow(exp, modulus)
}

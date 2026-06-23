use hex_literal::hex;
use num_bigint::{BigInt, Sign};
// pub struct ElGamal {
//     p: BigInt,
//     r: BigInt,
//     // s: BigInt,
//     // u: BigInt,
//     e: BigInt,
// }
const P: [u8; 32] = hex!("f33b6fb972a0b72515e45c391829e182ad8a9bdc0a64d3444d79c810ab863717");
const R: [u8; 32] = hex!("e66f544afcce68c5ef07b9a07b277585344a1db61376e831f73b9fbd5f44f715");

pub fn compute_x0() -> [u8; 8] {
    let e = BigInt::from(0x14u64);
    let p = BigInt::from_bytes_le(Sign::Plus, &P);
    let r = BigInt::from_bytes_le(Sign::Plus, &R);
    let x0 = r.modpow(&e, &p);

    let digits = x0.to_bytes_le().1;
    let mut out = [0u8; 8];
    let len = digits.len().min(8);
    out[..len].copy_from_slice(&digits[..len]);
    out
}

// impl ElGamal {
//     // const Q: [u8; 32] = hex!("f99db75c39d0db920a72ae1c8c9470c156c54d6e05b269a2a63c648855c39b0b");
//
//     // pub fn new() -> Self {
//     //     ElGamal {
//     //         p: BigInt::from_bytes_le(Sign::Plus, &ElGamal::P),
//     //         r: BigInt::from_bytes_le(Sign::Plus, &ElGamal::R),
//     //         e: BigInt::from(0x14u64),
//     //     }
//     // }
//
//     // pub fn compute_x0(&self) -> BigInt {
//     //     self.r.modpow(&self.e, &self.p)
//     // }
// }

pub const ADD: u64 = 0x9e3779b97f4a7c15;
const MUL1: u64 = 0xbf58476d1ce4e5b9;
const MUL2: u64 = 0x94d049bb133111eb;

pub fn next(state: u64) -> u64 {
    let s = state.wrapping_add(ADD);
    let mut z = s;
    z = (z ^ (z >> 30)).wrapping_mul(MUL1);
    z = (z ^ (z >> 27)).wrapping_mul(MUL2);
    z ^ (z >> 31)
}

pub fn step(state: &mut u64) {
    *state = state.wrapping_add(ADD);
}

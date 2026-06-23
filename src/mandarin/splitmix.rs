pub fn splitmix(state: u64) -> u64 {
    let new_state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = new_state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splitmix() {
        let step = splitmix(1234567);
        assert_eq!(step, 6457827717110365317);
    }
}

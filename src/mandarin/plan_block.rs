use crate::mandarin::splitmix::splitmix;

const SEED_ENC: u64 = 0x61f6868699c14dfa;
const BLOCK_UNIT_SIZE: u64 = 0x4000;

pub fn plan_block(decrypted_data_length: u64) -> (u64, Vec<u64>) {
    let mut state = SEED_ENC;
    let max_number_of_blocs = (decrypted_data_length / BLOCK_UNIT_SIZE) + 1;
    let mut block_sizes: Vec<u64> = Vec::new();
    let mut remaining_size = decrypted_data_length;

    for _ in 0..max_number_of_blocs {
        let number_of_blocs: u8 = ((state & 7) + 1) as u8;
        let size: u64 = number_of_blocs as u64 * BLOCK_UNIT_SIZE as u64;
        if size <= remaining_size {
            remaining_size -= size;
            block_sizes.push(size);
        } else {
            if remaining_size != 0 {
                block_sizes.push(size);
            }
            remaining_size = 0;
        }
        state = splitmix(state);
    }
    (state, block_sizes)
}

fn compare_block_plan(decrypted_len: u64) -> (Vec<u8>, usize, u64) {
    println!("decrypted_len: {decrypted_len}");
    let num_potential = ((decrypted_len & 0x3fff != 0) as u64) + (decrypted_len >> 0xe);
    println!("num_pot: {num_potential}");
    let mut sizes = vec![0u8; num_potential as usize];
    let mut state = SEED_ENC;
    for i in 0..num_potential as usize {
        sizes[i] = (state & 7) as u8 + 1;
        state = splitmix(state);
    }
    let mut leftover = decrypted_len;
    let mut real = 0usize;
    for i in 0..num_potential as usize {
        let span = sizes[i] as u64 * BLOCK_UNIT_SIZE as u64;
        real += 1;
        if leftover <= span {
            break;
        }
        leftover -= span;
    }
    (sizes, real, state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_block() {
        let (state, blocks) = plan_block(615480);
        assert_eq!(state, 0x0d5378b42a1eaf6c);
        let expected_sizes = [
            49152, 32768, 49152, 65536, 81920, 32768, 98304, 131072, 16384, 65536,
        ]
        .to_vec();
        assert_eq!(expected_sizes.len(), blocks.len());
        assert_eq!(expected_sizes, blocks);
    }
    #[test]
    fn test_plan_block_matches_compare_block_plan() {
        let decrypted_len = 615480u64;

        let (state_a, blocks_a) = plan_block(decrypted_len);
        let (sizes_b, real_b, state_b) = compare_block_plan(decrypted_len);

        // Reconstruit la liste de tailles "réelles" (en octets) à partir de
        // compare_block_plan, en ne gardant que les `real_b` premiers éléments
        // de `sizes_b`, et en les convertissant en octets (size * BLOCK_UNIT_SIZE).
        let blocks_b: Vec<u64> = sizes_b[0..real_b]
            .iter()
            .map(|&s| s as u64 * BLOCK_UNIT_SIZE)
            .collect();

        println!("plan_block        -> state={state_a:#x}, blocks={blocks_a:?}");
        println!("compare_block_plan -> state={state_b:#x}, blocks={blocks_b:?}, real={real_b}");

        assert_eq!(
            blocks_a, blocks_b,
            "les tailles de blocs diffèrent entre les deux implémentations"
        );
        assert_eq!(
            state_a, state_b,
            "le state final diffère entre les deux implémentations"
        );
    }
}

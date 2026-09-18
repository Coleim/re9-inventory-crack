use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::mandarin::{
    elgamal::compute_x0,
    plan_block::plan_block,
    splitmix::splitmix, // plan_block::plan_block,
};

const BASE_ID: u64 = 0x0110_0001_0000_0000;

pub fn crack_steamid(first_payload_bytes: [u8; 8], decrypted_data_length: u64) -> Option<u64> {
    let (initial_state, _) = plan_block(decrypted_data_length);

    let mut target_keystream = [0u8; 8];
    let x0 = compute_x0();

    for i in 0..8 {
        target_keystream[i] = first_payload_bytes[i] ^ x0[i];
    }

    let chunk_size = 1_000_000;
    let max_of_account_id = 0x100000000u64; // 4_294_967_296 (0xFFFFFFFF + 1)
    let number_of_chunks = max_of_account_id.div_ceil(chunk_size);

    let found_account_id = (0..number_of_chunks)
        .into_par_iter()
        .find_map_any(|chunk_index| {
            let start = chunk_index * chunk_size;
            let end = (start + chunk_size).min(max_of_account_id);

            let mut found = None;
            for account_id in start..end {
                let candidate_steamid = BASE_ID.wrapping_add(account_id);
                let mut state = initial_state.wrapping_add(!candidate_steamid);
                // 16 tours de chauffe
                for _ in 0..16 {
                    state = splitmix(state);
                }

                // Générer 8 bytes de keystream
                let mut mask = [0u8; 8];
                for i in 0..8 {
                    state = splitmix(state);
                    mask[i] = state as u8;
                }

                if mask == target_keystream {
                    found = Some(account_id);
                    break;
                }
            }
            found
        });

    if let Some(acc_id) = found_account_id {
        return Some(BASE_ID.wrapping_add(acc_id));
    }

    None
}

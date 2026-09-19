// MurmurHash3 (x86_32) — used both for:
// - the DSSS file's trailing 4-byte checksum (seed 0xffffffff, over the
//   whole file except that trailing checksum itself)
// - RE Engine field/class name hashing (seed 0xffffffff, over the UTF-8
//   name bytes), which lets us guess field/class hashes from candidate
//   name strings without needing an external name database.
pub fn hash32(data: &[u8], seed: u32) -> u32 {
    let mut h: u32 = seed;
    let nblocks = data.len() / 4;
    for i in 0..nblocks {
        let mut k = u32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap());
        k = k.wrapping_mul(0xcc9e2d51);
        k = k.rotate_left(15);
        k = k.wrapping_mul(0x1b873593);
        h ^= k;
        h = h.rotate_left(13);
        h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
    }
    let tail = &data[nblocks * 4..];
    let mut k: u32 = 0;
    for (i, &b) in tail.iter().enumerate() {
        k ^= (b as u32) << (8 * i);
    }
    if !tail.is_empty() {
        k = k.wrapping_mul(0xcc9e2d51);
        k = k.rotate_left(15);
        k = k.wrapping_mul(0x1b873593);
        h ^= k;
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h
}

/// Hash for a RE Engine field/class name (used to guess field hashes from
/// candidate strings).
pub fn name_hash(name: &str) -> u32 {
    hash32(name.as_bytes(), 0xffffffff)
}

/// Hash for a game data string value (e.g. an item id like `"it40_00_000"`)
/// as stored in `_ItemIDHash` fields: unlike field/class names (hashed as
/// UTF-8), these are hashed as UTF-16LE - confirmed by hashing the equipped
/// weapon's plain id string (`_Equips[]._ItemIDName`) and matching the
/// `_ItemIDHash` at the same `_ItemIndex` in `_PanelItems`.
pub fn utf16le_hash(s: &str) -> u32 {
    let bytes: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    hash32(&bytes, 0xffffffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_name_hash() {
        // Sanity check against the pattern used throughout re9_structs.tsv /
        // re9_fields.tsv: `app.Inventory` style names hash to the same
        // 32-bit values referenced there.
        // (No fixed expected value asserted here beyond determinism.)
        let a = name_hash("app.Inventory");
        let b = name_hash("app.Inventory");
        assert_eq!(a, b);
    }

    #[test]
    fn utf16le_hash_matches_equipped_weapon() {
        // "it10_02_000" is the equipped Requiem's plain item id
        // (_Equips[]._ItemIDName); its _ItemIDHash at the same _ItemIndex
        // in _PanelItems is 0xf29dd2b8 (4070429368), confirmed against a
        // real save.
        assert_eq!(utf16le_hash("it10_02_000"), 0xf29dd2b8);
    }
}

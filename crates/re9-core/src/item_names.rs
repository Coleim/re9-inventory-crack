//! Human-readable display names for known item ids.
//!
//! Unlike field/class names (`names.rs`) or item ids (`app.ItemID.Hash` in
//! `re9_enums.tsv`), there is no bundled localization table for item
//! display names. Entries here were identified manually by cross-
//! referencing real save files against in-game observations (see git log
//! for `inventory.rs`), starting with items whose id and quantity could be
//! confidently matched.
use std::collections::HashMap;
use std::sync::OnceLock;

static TABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

fn load() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("it20_00_003", "Bouteille vide"),
        ("it40_00_000", "Munition de pistolet"),
        ("it40_02_000", "Munition de Requiem"),
        ("it10_02_000", "Requiem"),
        ("it99_07_001", "Porte bonheur casse-dalle"),
    ])
}

/// Human-readable display name for a known item id (e.g. `"it40_00_000"`
/// -> `"Munition de pistolet"`), if known.
pub fn item_name(item_id: &str) -> Option<&'static str> {
    TABLE.get_or_init(load).get(item_id).copied()
}

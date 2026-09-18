//! Inventory slot discovery + editing.
//!
//! Field mapping confirmed against `references/re9_editor/re9_names.tsv`
//! (a community RE9 RSZ type dump, hashed with murmur3 the same way the
//! game hashes field names) and `references/re9_editor/dump11.txt`/
//! `diff.txt`. The relevant shape, under the RSZ tree:
//!
//! ```text
//! app.Inventory.SaveData#6dc40103         // one per inventory container
//!   _UserName#523cb981: String            // owner: "User00", "User01", ...
//!   _BoardTypeName#d772f792: String       // container name: "Hand", "ItemBox", "ShareItemBox"
//!   _PanelItems#dcc225ac: Array<app.Inventory.PanelItemSaveData#b00026aa>
//!     [i] _ItemIDHash#875dd7e9: U32       // item TYPE hash - murmur3(UTF-16LE(item_id), seed=0xffffffff)
//!         _Stock#955d3f51: S32            // <- the actual (reserve/carried) quantity
//!         _StoreOrder#069df450: S32       // UI grid sort order - NOT the quantity!
//!         _LoadingItems#9347ec5d: Array<app.Inventory.ContainItemSaveData#6b8482e9>
//!           [j] _AmountSaveData#902e5ff8 (app.ItemStockData.SaveData#491d1396)
//!                 _Stock#955d3f51: S32    // ammo loaded in a weapon's magazine
//!                 _ItemIDHash#875dd7e9    // ammo type loaded
//!               _ChamberStock#4d4a0375: S32  // round(s) in the chamber
//!         ... (attachments, position, display bits, etc.)
//! ```
//!
//! `_ItemIDHash` is `murmur3_x86_32(name.encode_utf16_le(), seed=0xffffffff)`
//! - confirmed by hashing the equipped weapon's plain id string
//! (`_Equips[]._ItemIDName`, e.g. `"it10_02_000"`) and matching the
//! `_ItemIDHash` at that same `_ItemIndex` in `_PanelItems`. This exact hash
//! is already present as enum values in the embedded `re9_enums.tsv` under
//! `app.ItemID.Hash` (and `app.ShopCustomizeItemID.Hash`), so we can resolve
//! it back to the plain item id string (e.g. `"it40_00_000"`) via
//! [`crate::schema::enum_name`]. A small, manually curated table
//! ([`crate::item_names`]) further maps a handful of known item ids to
//! human display names (e.g. `"Munition de pistolet"`).
//!
//! Note there can be several containers with the same `_BoardTypeName`
//! (e.g. "Hand") but different `_UserName` - these belong to different
//! characters/save-users, not duplicate/stale data.
//!
//! `_LoadingItems[]._AmountSaveData._Stock` (ammo loaded in a weapon's
//! magazine) is confirmed via `references/re9_editor/diff.txt`: it dropped
//! from 1 to 0 for the equipped "Requiem" pistol (`it10_02_000`) after
//! firing its last loaded round. `set_loaded_stock`/`set_chamber_stock`
//! edit these in place, same as the top-level `_Stock`.

use crate::rsz::{Class, Root, Value};

const CONTAINER_CLASS: u32 = 0x6dc40103;
const CONTAINER_USER_FIELD: u32 = 0x523cb981; // _UserName
const CONTAINER_NAME_FIELD: u32 = 0xd772f792; // _BoardTypeName
const ITEMS_FIELD: u32 = 0xdcc225ac; // _PanelItems
const ITEM_CLASS: u32 = 0xb00026aa;
const QUANTITY_FIELD: u32 = 0x955d3f51; // _Stock
const ITEM_ID_HASH_FIELD: u32 = 0x875dd7e9; // _ItemIDHash
const LOADING_ITEMS_FIELD: u32 = 0x9347ec5d; // _LoadingItems
const AMOUNT_SAVE_DATA_FIELD: u32 = 0x902e5ff8; // _AmountSaveData
const CHAMBER_STOCK_FIELD: u32 = 0x4d4a0375; // _ChamberStock

const ITEM_ID_ENUM_TYPE: &str = "app.ItemID.Hash";

#[derive(Debug, Clone)]
pub struct InventorySlot {
    /// Owner of this container, e.g. "User00", "User01" (co-op characters
    /// each have their own set of containers).
    pub owner: String,
    /// Human-readable container name, e.g. "Hand", "ItemBox", "ShareItemBox".
    pub container: String,
    /// Index of this container among all containers found (there can be
    /// several containers with the same name, even for the same owner).
    pub container_index: usize,
    /// Index of this item within its container's item array.
    pub item_index: usize,
    /// Hash of the item's type/id (`_ItemIDHash`).
    pub item_id_hash: u32,
    /// Resolved item id string (e.g. "it40_00_000"), if found in the
    /// embedded `app.ItemID.Hash` enum table. This is the game's internal
    /// item id, not a localized display name.
    pub item_id: Option<String>,
    /// Human-readable display name (e.g. "Munition de pistolet"), if known
    /// - see [`crate::item_names`]. Only a handful of items are mapped so
    /// far, identified manually from real saves.
    pub item_name: Option<&'static str>,
    /// Current stack quantity (`_Stock`).
    pub quantity: i32,
    /// Byte offset of the quantity field in the decrypted payload, for
    /// in-place editing via [`set_quantity`].
    pub quantity_offset: usize,
    /// Ammo loaded into this item (e.g. a weapon's magazine + chamber), if
    /// any (`_LoadingItems`).
    pub loaded: Vec<LoadedAmmo>,
}

/// Ammo loaded in a weapon (magazine stock + chamber), nested under an
/// [`InventorySlot`]'s `_LoadingItems[j]`.
#[derive(Debug, Clone)]
pub struct LoadedAmmo {
    /// Index within the parent item's `_LoadingItems` array.
    pub loaded_index: usize,
    /// Hash of the loaded ammo's type/id (`_AmountSaveData._ItemIDHash`).
    pub item_id_hash: u32,
    pub item_id: Option<String>,
    /// Human-readable display name, if known - see [`crate::item_names`].
    pub item_name: Option<&'static str>,
    /// Ammo loaded in the magazine (`_AmountSaveData._Stock`).
    pub stock: i32,
    pub stock_offset: usize,
    /// Round(s) in the chamber (`_ChamberStock`), separate from the
    /// magazine stock.
    pub chamber_stock: i32,
    pub chamber_stock_offset: usize,
}

fn field<'a>(c: &'a Class, hash: u32) -> Option<&'a Value> {
    c.fields.iter().find(|f| f.hash == hash).map(|f| &f.value)
}

fn as_string(v: &Value) -> Option<&str> {
    match v {
        Value::Str { s, .. } => Some(s.as_str()),
        _ => None,
    }
}

fn as_scalar(v: &Value) -> Option<(usize, &str)> {
    match v {
        Value::Scalar { off, text, .. } => Some((*off, text.as_str())),
        _ => None,
    }
}

fn as_class(v: &Value) -> Option<&Class> {
    match v {
        Value::Class(c) => Some(c),
        _ => None,
    }
}

/// Resolve an `_ItemIDHash` value to its plain item id string (e.g.
/// `"it40_00_000"`), via the embedded `app.ItemID.Hash` enum table.
pub fn resolve_item_id(item_id_hash: u32) -> Option<String> {
    crate::schema::enum_name(ITEM_ID_ENUM_TYPE, item_id_hash as i64).map(|s| s.to_string())
}

/// Resolve a human-readable display name for an item id, if known.
fn resolve_item_name(item_id: Option<&str>) -> Option<&'static str> {
    item_id.and_then(crate::item_names::item_name)
}

fn collect_loaded_ammo(item: &Class) -> Vec<LoadedAmmo> {
    let mut out = Vec::new();
    let Some(Value::Array { items, .. }) = field(item, LOADING_ITEMS_FIELD) else {
        return out;
    };
    for (loaded_index, entry) in items.iter().enumerate() {
        let Some(container_item) = as_class(entry) else {
            continue;
        };
        let Some(amount) = field(container_item, AMOUNT_SAVE_DATA_FIELD).and_then(as_class) else {
            continue;
        };
        let Some((stock_offset, stock_text)) =
            field(amount, QUANTITY_FIELD).and_then(as_scalar)
        else {
            continue;
        };
        let Ok(stock) = stock_text.parse::<i32>() else {
            continue;
        };
        let item_id_hash = field(amount, ITEM_ID_HASH_FIELD)
            .and_then(as_scalar)
            .and_then(|(_, t)| t.parse::<u32>().ok())
            .unwrap_or(0);
        let item_id = resolve_item_id(item_id_hash);
        let (chamber_stock, chamber_stock_offset) =
            match field(container_item, CHAMBER_STOCK_FIELD).and_then(as_scalar) {
                Some((off, text)) => (text.parse::<i32>().unwrap_or(0), off),
                None => (0, 0),
            };
        out.push(LoadedAmmo {
            loaded_index,
            item_id_hash,
            item_name: resolve_item_name(item_id.as_deref()),
            item_id,
            stock,
            stock_offset,
            chamber_stock,
            chamber_stock_offset,
        });
    }
    out
}

fn walk_class(c: &Class, out: &mut Vec<InventorySlot>, container_counter: &mut usize) {
    if c.hash == CONTAINER_CLASS {
        if let (Some(name_val), Some(Value::Array { items, .. })) =
            (field(c, CONTAINER_NAME_FIELD), field(c, ITEMS_FIELD))
        {
            if let Some(name) = as_string(name_val) {
                let owner = field(c, CONTAINER_USER_FIELD)
                    .and_then(as_string)
                    .unwrap_or("")
                    .to_string();
                let container_index = *container_counter;
                *container_counter += 1;
                for (item_index, item) in items.iter().enumerate() {
                    if let Value::Class(ic) = item {
                        if ic.hash == ITEM_CLASS {
                            let item_id_hash = field(ic, ITEM_ID_HASH_FIELD)
                                .and_then(as_scalar)
                                .and_then(|(_, t)| t.parse::<u32>().ok())
                                .unwrap_or(0);
                            if let Some((off, text)) =
                                field(ic, QUANTITY_FIELD).and_then(as_scalar)
                            {
                                if let Ok(quantity) = text.parse::<i32>() {
                                    let item_id = resolve_item_id(item_id_hash);
                                    out.push(InventorySlot {
                                        owner: owner.clone(),
                                        container: name.to_string(),
                                        container_index,
                                        item_index,
                                        item_id_hash,
                                        item_name: resolve_item_name(item_id.as_deref()),
                                        item_id,
                                        quantity,
                                        quantity_offset: off,
                                        loaded: collect_loaded_ammo(ic),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Recurse into every nested field, regardless of whether this class
    // matched, since containers can be nested inside other classes/arrays.
    for f in &c.fields {
        walk_value(&f.value, out, container_counter);
    }
}

fn walk_value(v: &Value, out: &mut Vec<InventorySlot>, container_counter: &mut usize) {
    match v {
        Value::Class(c) => walk_class(c, out, container_counter),
        Value::Array { items, .. } => {
            for it in items {
                walk_value(it, out, container_counter);
            }
        }
        _ => {}
    }
}

/// Walk every root and collect all inventory slots found.
pub fn list_inventory(roots: &[Root]) -> Vec<InventorySlot> {
    let mut out = Vec::new();
    let mut container_counter = 0usize;
    for root in roots {
        if let Ok(class) = &root.class {
            walk_class(class, &mut out, &mut container_counter);
        }
    }
    out
}

/// Overwrite an inventory slot's quantity (`_Stock`) in place in the
/// decrypted payload.
pub fn set_quantity(payload: &mut [u8], slot: &InventorySlot, quantity: i32) -> Result<(), String> {
    set_i32_at(payload, slot.quantity_offset, quantity)
}

/// Overwrite a weapon's loaded (magazine) ammo stock in place
/// (`_LoadingItems[j]._AmountSaveData._Stock`).
pub fn set_loaded_stock(payload: &mut [u8], loaded: &LoadedAmmo, stock: i32) -> Result<(), String> {
    set_i32_at(payload, loaded.stock_offset, stock)
}

/// Overwrite a weapon's chamber ammo stock in place
/// (`_LoadingItems[j]._ChamberStock`).
pub fn set_chamber_stock(payload: &mut [u8], loaded: &LoadedAmmo, stock: i32) -> Result<(), String> {
    if loaded.chamber_stock_offset == 0 {
        return Err("this loaded-ammo entry has no _ChamberStock field".to_string());
    }
    set_i32_at(payload, loaded.chamber_stock_offset, stock)
}

fn set_i32_at(payload: &mut [u8], offset: usize, value: i32) -> Result<(), String> {
    if offset + 4 > payload.len() {
        return Err(format!("offset {offset:#x} out of bounds"));
    }
    payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    Ok(())
}

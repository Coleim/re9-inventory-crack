//! Inventory slot discovery + editing.
//!
//! Field mapping confirmed against `references/re9_editor/re9_names.tsv`
//! (a community RE9 RSZ type dump, hashed with murmur3 the same way the
//! game hashes field names) and `references/re9_editor/dump11.txt`/
//! `diff.txt`. The relevant shape, under the RSZ tree:
//!
//! ```text
//! app.Inventory.SaveData#6dc40103         // one per inventory container
//!   _BoardTypeName#d772f792: String       // container name: "Hand", "ItemBox", "ShareItemBox"
//!   _PanelItems#dcc225ac: Array<app.Inventory.PanelItemSaveData#b00026aa>
//!     [i] _ItemIDHash#875dd7e9: U32       // item TYPE hash (not a per-instance id)
//!         _Stock#955d3f51: S32            // <- the actual quantity
//!         _StoreOrder#069df450: S32       // UI grid sort order - NOT the quantity!
//!         _LoadingItems#9347ec5d: Array<app.Inventory.ContainItemSaveData#6b8482e9>
//!           [j] _AmountSaveData#902e5ff8 (app.ItemStockData.SaveData#491d1396)
//!                 _Stock#955d3f51: S32    // ammo loaded in a weapon's magazine
//!               _ChamberStock#4d4a0375: S32  // bullet in the chamber
//!         ... (attachments, position, display bits, etc.)
//! ```
//!
//! We don't yet know how to resolve `_ItemIDHash` back to a human item name
//! (e.g. "9mm Ammo") - it doesn't match a simple murmur3 of the plain item
//! id strings seen elsewhere (e.g. `"it10_02_000"`). For now slots are
//! identified by container name + index + item index + item-type hash.
//!
//! Only the top-level `_Stock` (reserve/carried quantity) is exposed here
//! for now; the nested `_LoadingItems` (ammo loaded in a weapon) is a
//! likely follow-up.

use crate::rsz::{Class, Root, Value};

const CONTAINER_CLASS: u32 = 0x6dc40103;
const CONTAINER_NAME_FIELD: u32 = 0xd772f792;
const ITEMS_FIELD: u32 = 0xdcc225ac;
const ITEM_CLASS: u32 = 0xb00026aa;
const QUANTITY_FIELD: u32 = 0x955d3f51; // _Stock
const ITEM_ID_HASH_FIELD: u32 = 0x875dd7e9; // _ItemIDHash

#[derive(Debug, Clone)]
pub struct InventorySlot {
    /// Human-readable container name, e.g. "Hand", "ItemBox", "ShareItemBox".
    pub container: String,
    /// Index of this container among all containers found (there can be
    /// several containers with the same name).
    pub container_index: usize,
    /// Index of this item within its container's item array.
    pub item_index: usize,
    /// Hash of the item's type/id (`_ItemIDHash`). Not yet resolvable to a
    /// human name; can be used to group/compare slots of the same item type.
    pub item_id_hash: u32,
    /// Current stack quantity (`_Stock`).
    pub quantity: i32,
    /// Byte offset of the quantity field in the decrypted payload, for
    /// in-place editing via [`set_quantity`].
    pub quantity_offset: usize,
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

fn walk_class(c: &Class, out: &mut Vec<InventorySlot>, container_counter: &mut usize) {
    if c.hash == CONTAINER_CLASS {
        if let (Some(name_val), Some(Value::Array { items, .. })) =
            (field(c, CONTAINER_NAME_FIELD), field(c, ITEMS_FIELD))
        {
            if let Some(name) = as_string(name_val) {
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
                                    out.push(InventorySlot {
                                        container: name.to_string(),
                                        container_index,
                                        item_index,
                                        item_id_hash,
                                        quantity,
                                        quantity_offset: off,
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
    if slot.quantity_offset + 4 > payload.len() {
        return Err(format!(
            "offset {:#x} out of bounds",
            slot.quantity_offset
        ));
    }
    payload[slot.quantity_offset..slot.quantity_offset + 4]
        .copy_from_slice(&quantity.to_le_bytes());
    Ok(())
}

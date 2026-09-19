# Resident Evil 9 Save File Editor

## Goal

Provide an easy way to decrypt, edit and re encrypt a RE9 Save file.

Endview would be to have a UI, where you input a save file and and Optional Steam Id
The Tool will then:
- Get the SteamId from the file, if not passed as input.
- Decrypt the saved file
- Display the Inventory slots of the Saved File.
- Display other Save information (nicely displayed, not editable for now)

Then the UI will allow to:
- Modify the Inventory slot quantities
- Export the new modified saved file to be used in the game.


## Sort term Plan

[x] Via CLI, allow to : decrypt / encrypt a saved file, given as parameter (inputfile.bin), then save it into a new file (inputfile_new.bin) 
[ ] Provide a UI that will allow to do the same, asking for file, steam id, clicking on "Edit" or something, will decrypt the dsave file, display basic info, then a button "Export Save File" will reencrypt the file
[ ] Via UI, be able to display, for a given save file, the list of the items in the inventory, with their quantity
[ ] The SteamID must be displayed/persisted somewhere during the process (CLI and UI), so it doesn't need to be re-cracked/re-entered manually when it's time to re-encrypt an edited save. CLI idea: write a sidecar file (e.g. `<output>.steamid`) next to the decrypted payload when `decrypt`/`crack` resolves it, and have `encrypt` auto-read it if `--steamid` is omitted (still overridable). UI idea: keep it in the app's session state after cracking/input, and show it in the UI (copyable) for reuse.

## Session notes / where we are


Decrypt function analysis and planning encrypt implementation
ses_f4b42b495ffeuUF9Wq4NtAV9cI

Progress so far (see `git log` on `main` for full detail):
- Workspace restructured into `crates/re9-core` (lib) + `crates/re9-cli`
  (bin `re9`), in prep for a future Leptos web frontend.
- CLI done: `decrypt`, `encrypt`, `crack`, `roundtrip`, `dump`, `paths`,
  `targets`, `set-path`, `diff`, `hash`, `inventory`, `set-quantity`,
  `set-loaded-quantity`, `set-chamber-quantity`. See `README.MD` for usage
  of each. Still missing: SteamID persistence between `decrypt`/`crack` and
  `encrypt` (see short-term plan item above).
- `re9_core::dsss::build()` assembles a full loadable save file (header +
  `cipher::encrypt` + length + murmur3 checksum). Byte-identical to the
  original file except for two known/accepted gaps (not required for the
  game to load it): a per-block 8-byte reserved/unknown field in the
  ElGamal key bank, and the trailing 128-byte RSA key-wrap blob (unneeded
  since we already recover the SteamID by brute force instead of via RSA).
- Ported a full, generic, self-describing RSZ parser/editor into
  `re9-core::rsz` (schema-assisted resync via embedded `re9_fields.tsv`/
  `re9_structs.tsv`, copied from `references/re9_editor_2/`), plus a full
  field/class name database (`re9-core::names`, embedding
  `re9_names.tsv` from the older `references/re9_editor/` iteration,
  195740 entries) so `dump`/`paths`/`diff`/`targets` show real names
  (`_InventoryData`, `_PanelItems`, `_Stock`, ...) instead of hex hashes.
- Inventory structure reverse-engineered empirically (by diffing real save
  pairs: shoot-then-reload, and two separate exports with known real
  inventories) and implemented in `re9_core::inventory`:
  - Container: class `0x6dc40103` (`app.Inventory.SaveData`), owner field
    `_UserName` (`0x523cb981`, e.g. `"User00"`/`"User01"` - co-op
    characters, NOT duplicate data), name field `_BoardTypeName`
    (`0xd772f792`, e.g. `"Hand"`, `"ItemBox"`, `"ShareItemBox"`).
  - Item slot: class `0xb00026aa` (`app.Inventory.PanelItemSaveData`),
    quantity field `_Stock` (`0x955d3f51`) - NOT `_StoreOrder`
    (`0x069df450`), which is just the UI grid sort order (this was an
    earlier bug, now fixed). Item type hash: `_ItemIDHash` (`0x875dd7e9`)
    = `murmur3_x86_32(item_id.encode_utf16_le(), seed=0xffffffff)`;
    resolved back to the plain item id (e.g. `"it40_00_000"`) via the
    `app.ItemID.Hash` enum already present in the embedded
    `re9_enums.tsv` - no brute forcing needed.
  - Weapons with ammo loaded also expose `_LoadingItems[]` entries
    (magazine `_Stock` + separate `_ChamberStock`), confirmed via a real
    shoot: the equipped weapon's loaded stock dropped by exactly one round
    fired.
  - `re9-core::item_names`: small, manually curated item-id -> display-name
    table (currently 5 entries: "Bouteille vide", "Munition de pistolet",
    "Requiem", "Munition de Requiem", "Porte bonheur casse-dalle"),
    identified by cross-referencing real saves against in-game
    observations. 3 items remain unidentified (candidates for "Remede
    injectable", "ferraille", "melange d'herbe").
  - `set_quantity()`/`set_loaded_stock()`/`set_chamber_stock()` edit in
    place; verified edit -> `encrypt` -> `decrypt` -> `inventory`
    round-trips correctly, including on real user-provided saves.

Not started yet:
- Leptos web UI (CSR, static export via `trunk`, deployable to GitHub
  Pages). Plan: manual Web Worker pool for SteamID brute force (no
  `SharedArrayBuffer`/COOP-COEP, since GitHub Pages can't set those
  headers) - each worker = independent WASM instance scanning a shard of
  the ~4.3B SteamID space.
- SteamID persistence between CLI commands (see short-term plan item).
- Filling in the remaining 3 unidentified item ids, and more generally
  growing `item_names` as more items get identified.


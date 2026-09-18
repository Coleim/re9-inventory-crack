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

## Session notes / where we are


Decrypt function analysis and planning encrypt implementation
ses_f4b42b495ffeuUF9Wq4NtAV9cI

Progress so far (see `git log` on `main` for full detail):
- Workspace restructured into `crates/re9-core` (lib) + `crates/re9-cli`
  (bin `re9`), in prep for a future Leptos web frontend.
- CLI done: `decrypt`, `encrypt`, `crack`, `roundtrip`, `dump`, `paths`,
  `targets`, `set-path`, `diff`, `hash`, `inventory`, `set-quantity`.
  See `README.MD` for usage of each.
- `re9_core::dsss::build()` assembles a full loadable save file (header +
  `cipher::encrypt` + length + murmur3 checksum). Byte-identical to the
  original file except for two known/accepted gaps (not required for the
  game to load it): a per-block 8-byte reserved/unknown field in the
  ElGamal key bank, and the trailing 128-byte RSA key-wrap blob (unneeded
  since we already recover the SteamID by brute force instead of via RSA).
- Ported a full, generic, self-describing RSZ parser/editor into
  `re9-core::rsz` (schema-assisted resync via embedded `re9_fields.tsv`/
  `re9_structs.tsv`, copied from `references/re9_editor_2/`). No name
  database (`re9_names.tsv`) was available, so fields display as hex
  hashes; `murmur3::name_hash` can be used to test candidate name strings.
- Inventory structure reverse-engineered empirically (by diffing
  `test001_with_munitions.bin` vs `test_001_after_reload.bin`, a real
  shoot-then-reload save pair) and implemented in `re9_core::inventory`:
  - Container: class `0x6dc40103`, name field `0xd772f792` (e.g. `"Hand"`,
    `"ItemBox"`, `"ShareItemBox"`).
  - Item slot: class `0xb00026aa`, quantity field `0x069df450` (confirmed:
    matches exact expected deltas, and a new magazine slot appearing after
    reload). Item *type* identity (e.g. "9mm Ammo") is NOT yet known - no
    plain item-id string was found on the item struct itself; the per-item
    `0x875dd7e9` field is a per-instance serial, not a type hash.
  - `set_quantity()` edits in place; verified edit -> `encrypt` ->
    `decrypt` -> `inventory` round-trips correctly.

Not started yet:
- Leptos web UI (CSR, static export via `trunk`, deployable to GitHub
  Pages). Plan: manual Web Worker pool for SteamID brute force (no
  `SharedArrayBuffer`/COOP-COEP, since GitHub Pages can't set those
  headers) - each worker = independent WASM instance scanning a shard of
  the ~4.3B SteamID space.
- Resolving inventory item *type* (name/id), only quantity is known so far.


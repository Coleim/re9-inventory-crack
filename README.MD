# Resident Evil 9 Save File Editor

Decrypt, inspect, and re-encrypt Resident Evil Requiem ("RE9") DSSS save files.

Save files are located: 
C:\Users\Clement\AppData\Roaming\GSE Saves\3764200\remote\win64_save


See `PLAN.MD` for the overall project goals/roadmap.

## Building

```sh
cargo build --release
```

The CLI binary is at `target/release/re9`.

## CLI usage

### Decrypt a save file

```sh
re9 decrypt <file.bin> [--steamid <id>] [--out <output.bin>]
```

- If `--steamid` is omitted, it is recovered automatically by brute force
  (~4.3B combinations, a few seconds on a multi-core machine).
- If `--out` is omitted, the output defaults to `<file>_new.bin`.
- Produces the raw decrypted save payload (not a valid save file on its own).

```sh
re9 decrypt data011Slot.bin --steamid 76561197960285355
# -> data011Slot_new.bin (decrypted payload)
```

### Re-encrypt a decrypted payload back into a loadable save file

```sh
re9 encrypt <decrypted.bin> --steamid <id> [--out <output.bin>]
```

```sh
re9 encrypt data011Slot_new.bin --steamid 76561197960285355 --out data011Slot_edited.bin
```

### Recover the SteamID from a save file

```sh
re9 crack <file.bin>
```

### Sanity check: decrypt -> re-encrypt -> decrypt and verify

```sh
re9 roundtrip <file.bin> [--steamid <id>]
```

### List inventory slots (with quantities)

```sh
re9 inventory <decrypted.bin> [--container <name>] [--owner <name>]
```

```sh
re9 inventory data011Slot_new.bin
# User00/Hand#1 [0]  qty=2  item=it20_00_003 (item_id_hash=0x377adaab)  @0x86874
# User00/Hand#1 [1]  qty=1  item=it00_01_000 (item_id_hash=0x084fcc97)  @0x86938
# User00/Hand#1 [4]  qty=1  item=it10_02_000 (item_id_hash=0xf29dd2b8)  @0x86b90
#     loaded[0]  item=it40_02_000 (item_id_hash=0xf9977e4a)  stock=4 @0x86be8  chamber=0 @0x86c1c
# User00/ItemBox#2 [0]  qty=1  item=it99_07_000 (item_id_hash=0x8aa10656)  @0x872cc
# ...
```

Each slot is identified by its owner (`User00`, `User01`, ... - co-op saves
have one set of containers per character), container name (`Hand`,
`ItemBox`, `ShareItemBox`, ...), the index of that container (since a given
owner can have several containers with the same name), and the item's index
within it. `item` is the item's internal id (e.g. `it40_00_000`), resolved
from `item_id_hash` via the embedded `app.ItemID.Hash` enum table; there is
no further mapping to the item's localized display name (e.g. "9mm Ammo")
yet.

Weapons that have ammo loaded show one or more indented `loaded[j]` lines:
`stock` is the ammo loaded in the magazine, `chamber` is the round loaded in
the chamber (separate from the magazine); both are independently editable
(see below).

### Edit an inventory slot's quantity

```sh
re9 set-quantity <decrypted.bin> <container> <container_index> <item_index> <quantity>
```

```sh
re9 set-quantity data011Slot_new.bin Hand 1 6 99
# Hand#1 [6]  qty: 12 -> 99  @0x873cc
```

### Edit a weapon's loaded ammo (magazine / chamber)

```sh
re9 set-loaded-quantity <decrypted.bin> <container> <container_index> <item_index> <loaded_index> <quantity>
re9 set-chamber-quantity <decrypted.bin> <container> <container_index> <item_index> <loaded_index> <quantity>
```

```sh
re9 set-loaded-quantity data011Slot_new.bin Hand 1 4 0 5
# Hand#1 [4].loaded[0]  stock: 4 -> 5  @0x86be8
```

This edits the decrypted payload in place; re-encrypt it afterwards with
`re9 encrypt` to produce a loadable save file.

### Inspect the raw save structure (advanced / debugging)

```sh
re9 dump <decrypted.bin> [--grep <text>] [--out <file>]     # full tree, human readable
re9 paths <decrypted.bin> [--grep <text>]                    # every field as a flat `path = value`
re9 targets <decrypted.bin> [--grep <text>]                   # only in-place editable scalar fields
re9 set-path <decrypted.bin> <path> <value>                   # edit any scalar field by path
re9 diff <a.bin> <b.bin> [--out <file>]                       # compare two decrypted payloads
re9 hash <name>                                               # murmur3 hash of a candidate field/class name
```


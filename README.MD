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
re9 inventory <decrypted.bin> [--container <name>]
```

```sh
re9 inventory data011Slot_new.bin
# Hand#1 [0]  qty=2  item_id_hash=0x377adaab  @0x86e34
# Hand#1 [1]  qty=1  item_id_hash=0x084fcc97  @0x86ef8
# ItemBox#2 [0]  qty=1  item_id_hash=0x8aa10656  @0x8788c
# ...
```

Each slot is identified by its container name (`Hand`, `ItemBox`,
`ShareItemBox`, ...), the index of that container (since several containers
can share the same name), and the item's index within it. `item_id_hash` is
a hash of the item's type (not yet resolvable to a human-readable name, but
stable/comparable across slots and saves).

### Edit an inventory slot's quantity

```sh
re9 set-quantity <decrypted.bin> <container> <container_index> <item_index> <quantity>
```

```sh
re9 set-quantity data011Slot_new.bin Hand 1 6 99
# Hand#1 [6]  qty: 12 -> 99  @0x873cc
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


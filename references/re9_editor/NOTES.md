## le format
- 2 saves, 628028 o pile pareil -> format taille fixe
- `xxd` -> magic `DSSS` (saves RE Engine). header 16 o : `DSSS | ver=2 | flags=0x10 | 0`
- entropie body = 7.9997 -> chiffré (pas compressé)
- `628012 % 16 != 0` mais footer 12 o à la fin -> `u64 decrypted_len(615480) + u32 hash`
- payload = 628000 = 39250*16 -> aes par blocs confirmé
- u32 final = murmur3 du fichier
- 39250 blocs tous distincts -> pas ECB
- 2 saves identiques sur 512 premiers o -> clé = compte, pas fichier

## reverse du binaire
- black-box sur les .bin s'arrête à "aes par blocs + clé compte". l'algo exact (seeds, prime elgamal...) est PAS devinable depuis le ciphertext. faut le binaire du jeu
- charger l'exe re9 dans un désassembleur (ghidra/ida)
- chercher la string ascii `DSSS` -> xrefs -> la fonction qui écrit/lit la save = le serializer + le crypto
- **on devine pas les primitives, on les RECONNAÎT par leurs constantes magiques** (plugin type FindCrypt, ou recherche manuelle) :
  - SplitMix64 : `9e3779b97f4a7c15`, `bf58476d1ce4e5b9`, `94d049bb133111eb`
  - Murmur3 x86_32 : `cc9e2d51`, `1b873593`, `85ebca6b`, `c2b2ae35`
  - CityHash64 : `c3a5c85c97cb3127`, `b492b66fbe98f273`, `9ae16a3b2f90404f`
  - AES : la S-box (`63 7c 77 7b f2 6b...`) ou les instr `aesenc`/`aesenclast`
  - elgamal/rsa : les grands premiers (256/1024 bits) en dur dans la data, exposants `0x14` et `0x10001` (65537), entourés d'une boucle modexp (square-and-multiply)
- une fois les primitives ID, le code autour donne le câblage : quelle seed entre dans splitmix, comment le steamid est injecté (appel `GetSteamID()` puis `not`/`add`), l'ordre des opés
- confirmation dynamique : debugger, breakpoint sur la fonction de save, dump key/iv/seed au runtime
- tout ce qu'on lit dans le binaire se recoupe avec les octets du fichier (alignement, footer, blocs) -> les 2 analyses se rejoignent au milieu

## flag 0x10 = MANDARIN (re9)
- le flags est un champ de bits. 0x10 = le schéma "mandarin"
- params re9 : seeds=(seed_rsa=0, seed_enc=0x61f6868699c14dfa), clé = SteamID64

## chiffrement mandarin
- prng SplitMix64 (piège : sortie réinjectée comme état, `etat = next(etat)`)
- prng découpe en blocs taille variable (1..8 * 0x4000)
- `etat += !SteamID` <- le steamid rentre ici
- par bloc : prng sort key+iv -> blanchit meta (xor) -> AES-128-OFB -> check cityhash64
- meta 0x210 : 4 paires ElGamal (key/iv re-chiffré) + checksum + 8 o inutiles
- elgamal = vrai verrou de propriété (le jeu compare key/iv prng vs key/iv elgamal)
- entier rsa 0x80 à la fin = tag, encode !SteamID

## crack du steamid
- decrypt par candidat = trop lent. faut un oracle pas cher
- 1er octet chiffré = x0 blanchi. **x0 = R^e mod P est constant** (R,P,e fixes) -> clair connu gratis
- `masque[j] = fichier[j] XOR x0[j]` (8 octets), calculé sans clé
- le masque dépend que du steamid (via `etat += !SteamID`)
- steamid64 = `0x0110000100000000 + account_id` (32 bits) -> 4.3 milliards max
- test/candidat = ~24 pas splitmix, compare 8 o. rayon
- `crack data011 -> 76561197960285355` en 0.09s
- 8 o = 64 bits -> match unique, pas de faux positif

## rsz / dump
- clair = RSZ **auto-décrivant** : chaque field = `[hash u32][field_type i32][value]`
- types : 2 Bool, 3-a entiers, b/c floats, f String(utf16), 10 Struct(blob), 11 Class, -1 Array
- Class = `[num_fields][hash][fields]`. Array = `[member_type][member_size][len][array_type]` (+marqueur `0xffeeffee` si array de classes)
- piège align : Struct s'aligne au **multiple de sa taille** (24 -> pas une puissance de 2, le `& !(n-1)` casse, faut `((p+n-1)/n)*n`)
- pas besoin de base de types pour parser, juste pour les NOMS (sinon hash)
- top-level = suite de roots, signature `<native_hash> 01000000 3c7737e1`. je segmente là-dessus -> un root cassé tue pas les autres (33/38 OK, reste en scan de strings)
- `dump <dec> [-o f] [--grep X]` sort l'arbre complet avec offsets
- inventaire = Array<Class>[21], chaque item = id string + u64 + bool + sous-array de props

## noms de champs (hash -> nom)
- les hashes = **murmur3 x86_32 du nom, seed 0xffffffff** (sur les octets ascii du nom)
- vérifié : `_Data`=695a3627, `_KeyStr`=c40ada6c, `_SaveCount`=45157494, `_Stock`=955d3f51
- table générée depuis le dump de types RE9 de la commu (rszre9.json, fait via REFramework) : je hash tous les noms de champs+types -> `re9_names.tsv` (195740 entrées), chargé par l'outil
- `hash <nom>` calcule le murmur3 d'un nom deviné pour le retrouver dans le dump
- maintenant le dump montre `_Backups`, `_KeyStr`, `_AmountSaveData._Stock`, types `app.DetailSearchContext.SaveData` etc.

## diff
- comparer par OFFSET = inutile (63% diffèrent, tout décalé). comparer par CHEMIN rsz = propre
- j'aplatis chaque arbre en `chemin -> valeur` et je compare -> **29 vraies diffs** seulement
- `diff a b [-o f]` : ~ = changé (avec offset dans A), +/- = présent que d'un côté
- ex trouvé : `_SaveCount 4->5`, `_SerialNumber 71->73`, `..._AmountSaveData._Stock 1->0` (= une quantité d'item)

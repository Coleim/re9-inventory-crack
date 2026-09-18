

le fichier suit le format Lime: 
https://github.com/mi5hmash/LimebrellaSharp/blob/6f98841f/.resources/patterns/DSSS_Lime%20(v1).hexpat

######### CRACK DU STEAM ID

Le fichier est chiffré avec AES-OFB. En OFB :
ciphertext = plaintext XOR keystream

ciphertext = fichier d'input


Les 8 premiers bytes sont connu et constant.

ciphertext[0..8] = plaintext XOR keystream
                 = x0 XOR keystream

x0 = R^e mod P
R, P, e        ## from ghidra - re9.exe (constantes ElGamal hardcodées)

R = 0xe66f544afcce68c5ef07b9a07b277585344a1db61376e831f73b9fbd5f44f715
e = 0x14
P = 0xf33b6fb972a0b72515e45c391829e182ad8a9bdc0a64d3444d79c810ab863717


ciphertext[j] = plaintext[j] XOR keystream = x0[j] XOR keystream
ciphertext[j] XOR x0[j] = x0[j] XOR x0[j] XOR keystream = keystream 

keystream[j] = ciphertext[j] XOR x0[j]  


donc on va calculer le keystream attendu ; appellons le target_keystream

let mut target_keystream = [0u8; 8];

for j in 0..8 {
    target_keystream[j] = ciphertext[j] ^ x0[j];
}



Le keystream est généré par SplitMix64 à partir du SteamID 
keystream ≠ steamid
keystream = SplitMix64(steamid)


SplitMix demarre a SEED_ENC
SEED_ENC = 0x61f6868699c14dfa  ## from ghidra - re9.exe

Puis il plannifie les bloc avant de demarrer l'encryptage avec le Steam Id ; donc quand on decrypt on doit partir de la valeur du state (seed) APRES planifiage des blocs

Le plannifiage de bloc en fait, quand on encrypt, on va prendre un nombre de blocs aleatoire N de taille 0x4000 de bytes a encrypter 
Ce N est calculé grace a Splitmix, et c'est le nouveau "state"
nombre_de_bloc = (state & 7) + 1   ## from ghidra - re9.exe
state = splitmix(state)        ## from ghidra - re9.exe (algo reconnu par constante 0x9e3779b97f4a7c15)
la taille de la data a encryptée fait donc nombre_de_bloc*0x4000 

state = SEED_ENC
taille[0] = (state & 7) + 1  → entre 1 et 8
state = splitmix(state)
taille[1] = (state & 7) + 1  → entre 1 et 8
state = splitmix(state)

Taille = taille des données avant chiffrement
On prend ce nombre de bytes dans le fichier clair
On les chiffre avec AES-OFB
Le résultat chiffré a la même taille (OFB ne change pas la taille)


  SteamId = 0x0110000100000000 + account_id 
  il faut donc trouver l'account id (0x00000000..0xFFFFFFFF)

Pour chaque steamid candidate:
  on part du dernier state retourné par la planification des blocs
  => state = plan_block_state + !steamid ( C'est une décision du développeur Capcom — injecter le SteamID via addition après l'avoir inversé (!steamid) )  ## from ghidra - re9.exe (câblage injection SteamID)

  Ensuite on fait tourner SplitMix pendant 16 fois a vide, pour avoir un autre state ( # Get this from Ghidra ) ## from ghidra - re9.exe

  Puis on genere 8 bytes de keystream que l'on va comparer au mask 
  mask[i] = s as u8              ## from ghidra - re9.exe



  // 1. Partir de initial_state (résultat de block_plan)
  let mut s = initial_state.wrapping_add(!steamid);

  // 2. 16 tours de chauffe
  for _ in 0..16 {
      s = splitmix::next(s);
  }

  // 3. Générer 8 bytes de keystream
  let mut mask = [0u8; 8];
  for i in 0..8 {
      s = splitmix::next(s);
      mask[i] = s as u8;
  }

  // 4. Comparer
  if mask == target_keystream {
      // trouvé !
  }


--------------------------------------------------------

## DECHIFFRAGE DU FICHIER


Le chiffrement utilisé est AES OFB mode : https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#Output_feedback_(OFB)

1. prepare blocs (et retourne la taille de chaque bloc)

2. let mut s = initial_state.wrapping_add(!steamid);

3. Dechiffrer.


AES = use aes::Aes128;

pour chaque bloc Mandarin :

calculer les clés
    for i in 0..16 {
      let state = splitmix(state)
       key[i] = state as u8;
       iv[i] = state >> 8 as u8;
    }

Structure du fichier:
buf[0..512]      → keyFragmentsBank (blanchis par splitmix)
buf[512..528]    → checksum (0x200..0x208 = 8 bytes + 8 bytes padding ?)
buf[528..]       → segmentData chiffré  ← ce qu'on veut déchiffrer

on "deblanchit" de 0 a 528 
keyFragmentsBank: 512 bytes
resultat = byte 0 du keyFragmentsBank XOR splitmix(state)

checksum de 512 a 528
resultat = byte 0 du checksum XOR splitmix(state)


    
tant que totalbyte < N * 0x4000 :
    keystream = AES(IV_courant, clé)
    plaintext[totalbyte..totalbyte+16] = ciphertext[totalbyte..totalbyte+16] XOR keystream
    IV_courant = keystream   // ← OFB : le keystream devient le nouvel IV
    totalbyte += 16

OU

let mut cipher = Aes128Ofb::new(&k.into(), &iv.into());
cipher.apply_keystream(&mut buf[528..528 + block_size]);

-------------------------------------------

Lecture du fichier de facon "normale"


Scanner et trouver tous les roots via la signature <native_hash> 01000000 3c7737e1

[hash u32][field_type i32][value]
<native_hash> 01000000 3c7737e1
for class : `[hash][field type][number of fields][garbage?]`



├── rsz/                # NOUVEAU : parsing de la structure de save décryptée
│   ├── mod.rs
│   ├── node.rs         # les types FieldNode/ClassNode/StructNode/ArrayNode
│   ├── reader.rs        # le parseur qui lit les octets → arbre de nœuds
│   ├── writer.rs        # le sérialiseur arbre → octets (pour réencoder)
│   └── names.rs         # chargement du tsv hash→nom, lookup



// 1. Lecture
let raw = std::fs::read("save.dat")?;
let save_file = dsss::file::parse(&raw)?;                  // header + payload chiffré
let decrypted = mandarin::cipher::decrypt(&save_file.payload, id, save_file.decrypted_data_len);
let mut tree = rsz::reader::read(&decrypted)?;              // Vec<Node>, plusieurs roots

// 2. Édition
tree.get_mut("...weapon1...")?.set_i32(12);

// 3. Réécriture (le chemin inverse)
let new_decrypted = rsz::writer::write(&tree)?;              // Vec<u8> ré-encodé
let new_payload = mandarin::cipher::encrypt(&new_decrypted, id);  // ⚠️ il te faut une fonction encrypt symétrique
let new_save_file = dsss::file::build(header_modifié, new_payload);
std::fs::write("save_edited.dat", new_save_file.to_bytes())?;

-----

let tree = reader::read(&decrypted)?;          // arbre générique complet, intact

let mut inventory = Inventory::from_node(&tree); // vue typée, juste pour la partie inventaire
inventory.slots[3].stock = 12;

let new_tree = inventory.patch_into(tree);      // ne touche QUE les nœuds inventaire dans une copie de l'arbre
let bytes = writer::write(&new_tree)?;





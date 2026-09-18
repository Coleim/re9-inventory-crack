// ## DECHIFFRAGE DU FICHIER
//
// 1. prepare blocs (et retourne la taille de chaque bloc)
//
// 2. let mut s = initial_state.wrapping_add(!steamid);
//
// 3. Dechiffrer.
//
//
// AES = use aes::Aes128;
//
// pour chaque bloc Mandarin :
//
// calculer les clés
//     for i in 0..16 {
//       let state = splitmix(state)
//        key[i] = state as u8;
//        iv[i] = state >> 8 as u8;
//     }
//
// Structure du fichier:
// buf[0..512]      → keyFragmentsBank (blanchis par splitmix)
// buf[512..528]    → checksum (0x200..0x208 = 8 bytes + 8 bytes padding ?)
// buf[528..]       → segmentData chiffré  ← ce qu'on veut déchiffrer
//
// on "deblanchit" de 0 a 528
// keyFragmentsBank: 512 bytes
// resultat = byte 0 du keyFragmentsBank XOR splitmix(state)
//
// checksum de 512 a 528
// resultat = byte 0 du checksum XOR splitmix(state)
//
//
//
// tant que totalbyte < N * 0x4000 :
//     keystream = AES(IV_courant, clé)
//     plaintext[totalbyte..totalbyte+16] = ciphertext[totalbyte..totalbyte+16] XOR keystream
//     IV_courant = keystream   // ← OFB : le keystream devient le nouvel IV
//     totalbyte += 16
//
// OU
//
// let mut cipher = Aes128Ofb::new(&k.into(), &iv.into());
// cipher.apply_keystream(&mut buf[528..528 + block_size]);
//
//

use crate::mandarin::{elgamal::ElGamal, plan_block::plan_block, splitmix::splitmix};
use aes::cipher::StreamCipher;
use ofb::cipher::KeyIvInit;
type Aes128Ofb = ofb::Ofb<aes::Aes128>;

const BANK_SIZE: usize = 512;
const BANK_CHECKSUM_SIZE: usize = 16;

pub fn decrypt(encrypted_data: &[u8], steamid: u64, decrypted_data_length: u64) -> Vec<u8> {
    let bank_len = BANK_CHECKSUM_SIZE + BANK_SIZE;
    if encrypted_data.len() < bank_len {
        panic!("Data is smaller than the minimal expected {} ", bank_len);
    }

    let (initial_state, blocks) = plan_block(decrypted_data_length);

    let decrypted_data_length = decrypted_data_length as usize;
    let mut state = initial_state.wrapping_add(!steamid);
    let mut output = vec![0u8; decrypted_data_length as usize];

    let mut start_idx = 0usize; // offset dans `output` (données utiles déchiffrées)
    let mut start_enc = 0usize; // offset dans `encrypted_data` (fichier chiffré, bank inclus)

    for bloc_idx in 0..blocks.len() {
        // Pour chaque bloc mandarin de taille N:
        // buf[0..512]      → keyFragmentsBank (blanchis par splitmix)
        // buf[512..528]    → checksum (0x200..0x208 = 8 bytes + 8 bytes padding ?)
        // buf[528..N]       → segmentData chiffré  ← ce qu'on veut déchiffrer
        //
        //       buf[N..N+512]      → keyFragmentsBank (blanchis par splitmix)
        // buf[512..528]    → checksum (0x200..0x208 = 8 bytes + 8 bytes padding ?)
        // buf[528..N]       → segmentData chiffré  ← ce qu'on veut déchiffrer
        //
        // Et on loop sur chaque blocs

        let mut aes_key = [0u8; 16];
        let mut iv = [0u8; 16];

        for i in 0..16 {
            state = splitmix(state);
            aes_key[i] = state as u8;
            iv[i] = (state >> 8) as u8;
        }

        // "Déblanchiement" du keyFragmentBank + checksum
        for _ in 0..(bank_len) {
            state = splitmix(state);
            // output[start_idx + i] = encrypted_data[start_idx + i] ^ state as u8;
        }

        let block_size = blocks[bloc_idx] as usize;

        // We want to decrypt from encrypted data from start + bank_len until ... start + bank_len + block_size
        // and copy that into output form start to len
        let bank_start_enc = start_enc;
        let data_start_enc = bank_start_enc + bank_len;
        let data_end_enc = data_start_enc + block_size;

        if data_end_enc > encrypted_data.len() {
            panic!(
                "block {bloc_idx}: out of bounds in encrypted_data (need {} bytes, have {})",
                data_end_enc,
                encrypted_data.len()
            );
        }

        // Nombre d'octets réellement utiles à écrire pour ce bloc
        // (le dernier bloc peut être plus grand que ce qui reste à produire,
        // à cause de l'arrondi à UNIT côté plan_block).
        let to_copy = block_size.min(decrypted_data_length - start_idx);

        // Déchiffrement AES-OFB dans un buffer temporaire, sur tout le bloc
        // (pour rester synchro sur le keystream), puis on ne copie que `to_copy`
        // octets utiles dans `output`.
        let mut tmp = vec![0u8; block_size];
        let mut cipher = Aes128Ofb::new(&aes_key.into(), &iv.into());
        cipher.apply_keystream_b2b(&encrypted_data[data_start_enc..data_end_enc], &mut tmp);

        output[start_idx..start_idx + to_copy].copy_from_slice(&tmp[..to_copy]);

        start_idx += to_copy;
        start_enc = data_end_enc;
    }
    output
}

// ## CHIFFRAGE DU FICHIER
//
// Inverse de `decrypt` : reconstruit, bloc par bloc, la "keyFragmentsBank"
// (clé/IV AES chiffrés en ElGamal + checksum CityHash64) puis chiffre les
// données en AES-128-OFB.
//
// Comme l'exposant éphémère ElGamal utilisé par le jeu est fixe (`e = 0x14`),
// la bank n'est pas aléatoire : elle est entièrement déterministe à partir du
// SteamID et du contenu du bloc. On peut donc reconstruire un fichier chiffré
// strictement identique à l'original à partir des données déchiffrées.
pub fn encrypt(decrypted_data: &[u8], steamid: u64) -> Vec<u8> {
    let bank_len = BANK_CHECKSUM_SIZE + BANK_SIZE;
    let decrypted_data_length = decrypted_data.len() as u64;

    let (initial_state, blocks) = plan_block(decrypted_data_length);
    let mut state = initial_state.wrapping_add(!steamid);
    let auth = ElGamal::new(!steamid);

    let total_len =
        bank_len * blocks.len() + blocks.iter().sum::<u64>() as usize;
    let mut output = vec![0u8; total_len];

    let decrypted_data_length = decrypted_data_length as usize;
    let mut start_idx = 0usize; // offset dans `decrypted_data`
    let mut start_enc = 0usize; // offset dans `output`

    for block_size in blocks {
        let block_size = block_size as usize;

        let mut aes_key = [0u8; 16];
        let mut iv = [0u8; 16];
        for i in 0..16 {
            state = splitmix(state);
            aes_key[i] = state as u8;
            iv[i] = (state >> 8) as u8;
        }

        // Nombre d'octets utiles de `decrypted_data` disponibles pour ce bloc
        // (le dernier bloc peut être plus grand que ce qu'il reste à écrire,
        // le surplus est simplement du padding à zéro).
        let to_copy = block_size.min(decrypted_data_length - start_idx);

        let mut block_buf = vec![0u8; block_size];
        block_buf[..to_copy].copy_from_slice(&decrypted_data[start_idx..start_idx + to_copy]);

        let checksum = cityhasher::hash::<u64>(&block_buf[..to_copy]);

        let mut cipher = Aes128Ofb::new(&aes_key.into(), &iv.into());
        cipher.apply_keystream(&mut block_buf);

        // Construction de la keyFragmentsBank : clé+IV chiffrés en ElGamal
        // (4 mots de 8 octets), suivis du checksum CityHash64 du bloc en
        // clair.
        let mut key_iv = [0u8; 32];
        key_iv[0..16].copy_from_slice(&aes_key);
        key_iv[16..32].copy_from_slice(&iv);

        let mut bank = vec![0u8; bank_len];
        for c in 0..4 {
            let mut word = [0u8; 8];
            word.copy_from_slice(&key_iv[c * 8..c * 8 + 8]);
            let (c0, c1) = auth.encrypt_word(word);
            bank[c * 128..c * 128 + 64].copy_from_slice(&c0);
            bank[c * 128 + 64..c * 128 + 128].copy_from_slice(&c1);
        }
        bank[BANK_SIZE..BANK_SIZE + 8].copy_from_slice(&checksum.to_le_bytes());

        // Blanchiment de la bank (clé/IV chiffrés + checksum)
        for byte in bank.iter_mut() {
            state = splitmix(state);
            *byte ^= state as u8;
        }

        output[start_enc..start_enc + bank_len].copy_from_slice(&bank);
        output[start_enc + bank_len..start_enc + bank_len + block_size].copy_from_slice(&block_buf);

        start_idx += to_copy;
        start_enc += bank_len + block_size;
    }

    output
}

#[cfg(test)]
mod test {
    use hex_literal::hex;

    use super::*;

    #[test]
    fn test_decrypt() {
        let ciphertext = hex!(
            "19f68843cca35f7cd912e3f9030dff09dcb1bd8dfea4622e21db4eb3b293fe0b43a71306f7a4adcae691dc457682c79d52dd9bd8e4d6177cda0c18e05590c31d4c54bb9fa10be8f74ce9a9e0b692b49da707194a1dd53a30db7c03c6f029b830000cb86c3273af96df01793c73272f922c3b5e943566d4e96e81de23d6d1bb6c4a78cc1dc36e09b49a777f439b829e89afc16340356985321d8bc3f021ac5ea0f7195afd8e4a512932424f097a87b76703970d3eb33416c8ec96eb55e08786cc290e50e9079bcfec4bce2e32c7359aa3314527da25eec35e89c537c47d932d0308d83ae50ba38a49b1b3584d79f0128cde36a1b0ec2c7e73390989d742ff39cfb32cf4fff96e20a572f765adf2722a0d4a6f2afbf76428334ff4581f8bb7f4a1ee5c8cb5753d564c085907e23c0bed5f1fc7c2031d499b59f7df67564c9f6b6adf281b3a67ba6b7fd5412de00f7ff8cb8723da32626604d8ce76678327f2dcfe2fc0426c655fdade12b44859da045d7b5a1b5b04035e98be17263c3b3b4fe970f087937006510b26a7f50c35c42521e3a786c47f2f5cbd9e5f13a752a2a5a25a7a156e8bbcba2e5782b697b6f86181913534651231c9d09547170bd2def297311f2c2b08d86ac1924d124b24d8565147b4a7db0a9d1286696da3376f6faad7065eac21b0b86a793d44c5270faaa4628a674bf29d8ae749f004790d73ea4cb38cf53b3fbc173a0d1845fc762373cd0df7ac47e714c67000afe12d12a54f51dd6f582449f8"
        );

        let result = decrypt(&ciphertext, 76561197960285355, 20);
        let str: String = result.iter().map(|b| format!("{:02x}", b)).collect();
        println!("Result : {}", str);
        assert_eq!(0, 0);
        // e5ce7c8a010000003c7737e127365a69110000000d000000940247928ed7a2390800000004000000470000003f2227030900000008000000a42fbb993dc7de089289d6a408000000040000009c6ec16f7ae91e780800000004000000021000019474154508000000040000000400000093336eb00800000004000000a00ed576494bd4ab08000000040000004cc931d0eca5f7e3070000000400000000000000a6715f510800000004000000ade256ce39e1ac9e0800000004000000dc46837a3c6ed9a3020000000100000000000000054a76d20200000001000000000000002d735f4c0800000004000000d65b4dc603c6f666010000003c7737e127365a69110000000400000074716160d0851f52020000000100000000000000eccfd0f9040000000100000000000000c941801effffffff1100000008000000030000000100000002000000cb5f9fa825a9952b0800000004000000891fbeea50c894c508000000040000000000000002000000cb5f9fa825a9952b0800000004000000b5612a4450c894c508000000040000000000000002000000cb5f9fa825a9952b08000000040000001578327950c894c5080000000400000000000000c934e6dfffffffff11000000080000000000000001000000280b0765010000003c7737e127365a6911000000050000007669484371d9202c100000001000000000000000db0bcb95ac82894ba78803fe24018e695ec0c742110000000500000081f45a4ed9f8ff65
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        const STEAMID: u64 = 76561197960285355;

        // Couvre un seul bloc, plusieurs blocs, et une taille non alignée sur
        // BLOCK_UNIT_SIZE (padding du dernier bloc).
        for len in [1usize, 16, 0x4000, 0x4000 + 1, 0x10000 + 123] {
            let decrypted: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();

            let encrypted = encrypt(&decrypted, STEAMID);
            let roundtrip = decrypt(&encrypted, STEAMID, len as u64);

            assert_eq!(
                roundtrip, decrypted,
                "roundtrip mismatch for len={len}"
            );
        }
    }
}

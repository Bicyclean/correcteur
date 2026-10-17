//! Hachage rapide pour les tables du lexique.
//!
//! La fonction par défaut de Rust (SipHash) résiste aux collisions provoquées
//! par un attaquant, ce qui ne sert à rien ici : les clés viennent des fichiers
//! de données. Le hachage multiplicatif ci-dessous (celui du compilateur Rust,
//! « FxHash ») est plusieurs fois plus rapide sur des mots courts, et les
//! recherches dans le lexique dominent le calcul des suggestions.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.add(u64::from_le_bytes(c.try_into().unwrap_or_default()));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(buf));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub type FxHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table() {
        let mut m: FxHashMap<String, u32> = FxHashMap::default();
        for (i, w) in ["maison", "maisons", "mais", "", "a", "éléphant"]
            .iter()
            .enumerate()
        {
            m.insert(w.to_string(), i as u32);
        }
        assert_eq!(m.get("maisons"), Some(&1));
        assert_eq!(m.get(""), Some(&3));
        assert_eq!(m.get("éléphant"), Some(&5));
        assert_eq!(m.get("maiso"), None);
    }
}

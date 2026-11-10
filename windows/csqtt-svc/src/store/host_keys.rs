//! SSH host key trust-on-first-use (TOFU) store (port of SshHostKeyStore.kt).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::store::settings_file::{load_json, save_json};

const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Computes an unpadded Base64 string from binary data (RFC 4648 without trailing '=').
pub fn base64_encode_unpadded(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() * 4 + 2) / 3);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(BASE64_TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(BASE64_TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push(BASE64_TABLE[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = data.len() - i;
    if rem == 1 {
        let n = (data[i] as u32) << 16;
        out.push(BASE64_TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_TABLE[((n >> 12) & 0x3f) as usize] as char);
    } else if rem == 2 {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(BASE64_TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(BASE64_TABLE[((n >> 6) & 0x3f) as usize] as char);
    }
    out
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[inline(always)]
fn ch(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

#[inline(always)]
fn maj(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

#[inline(always)]
fn sigma0(x: u32) -> u32 {
    x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
}

#[inline(always)]
fn sigma1(x: u32) -> u32 {
    x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
}

#[inline(always)]
fn gamma0(x: u32) -> u32 {
    x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
}

#[inline(always)]
fn gamma1(x: u32) -> u32 {
    x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
}

/// Pure Rust implementation of SHA-256 (FIPS 180-4).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = [
        0x6a09e667u32, 0xbb67ae85u32, 0x3c6ef372u32, 0xa54ff53au32,
        0x510e527fu32, 0x9b05688cu32, 0x1f83d9abu32, 0x5be0cd19u32,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            w[i] = gamma1(w[i - 2])
                .wrapping_add(w[i - 7])
                .wrapping_add(gamma0(w[i - 15]))
                .wrapping_add(w[i - 16]);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_var = h[7];

        for i in 0..64 {
            let t1 = h_var
                .wrapping_add(sigma1(e))
                .wrapping_add(ch(e, f, g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let t2 = sigma0(a).wrapping_add(maj(a, b, c));
            h_var = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_var);
    }

    let mut out = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

/// Computes the SSH host key fingerprint in standard OpenSSH format (`SHA256:<unpadded-base64>`).
pub fn fingerprint(key_blob: &[u8]) -> String {
    let digest = sha256(key_blob);
    let encoded = base64_encode_unpadded(&digest);
    format!("SHA256:{encoded}")
}

/// Canonical entry key format matching Kotlin `entryKey(host, port) = "${host.trim().lowercase()}:$port"`.
pub fn entry_key(host: &str, port: u16) -> String {
    format!("{}:{}", host.trim().to_lowercase(), port)
}

/// Trust-on-first-use store for SSH host keys backed by a JSON file.
#[derive(Debug, Clone)]
pub struct SshHostKeyStore {
    path: PathBuf,
}

pub type HostKeyStore = SshHostKeyStore;

impl SshHostKeyStore {
    /// Creates a store with the specified JSON file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Path to the backing JSON file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Retrieves the pinned fingerprint for host and port if present.
    pub fn get(&self, host: &str, port: u16) -> anyhow::Result<Option<String>> {
        let entry = entry_key(host, port);
        let map: HashMap<String, String> = load_json(&self.path)?;
        Ok(map.get(&entry).cloned())
    }

    /// Manually pins a fingerprint for host and port.
    pub fn set(&self, host: &str, port: u16, fingerprint: &str) -> anyhow::Result<()> {
        let entry = entry_key(host, port);
        let mut map: HashMap<String, String> = load_json(&self.path)?;
        map.insert(entry, fingerprint.to_string());
        save_json(&self.path, &map)
    }

    /// Removes a pinned host key entry.
    pub fn clear(&self, host: &str, port: u16) -> anyhow::Result<()> {
        let entry = entry_key(host, port);
        let mut map: HashMap<String, String> = load_json(&self.path)?;
        if map.remove(&entry).is_some() {
            save_json(&self.path, &map)?;
        }
        Ok(())
    }

    /// Returns all pinned host key entries.
    pub fn all_entries(&self) -> anyhow::Result<HashMap<String, String>> {
        load_json(&self.path)
    }

    /// Trust-on-first-use verification against key blob.
    ///
    /// Pins the key if no entry exists; accepts if matching; rejects if mismatched.
    pub fn verify(&self, host: &str, port: u16, key_blob: &[u8]) -> anyhow::Result<bool> {
        let presented = fingerprint(key_blob);
        self.verify_fingerprint(host, port, &presented)
    }

    /// Trust-on-first-use verification against precomputed fingerprint.
    pub fn verify_fingerprint(&self, host: &str, port: u16, presented: &str) -> anyhow::Result<bool> {
        let entry = entry_key(host, port);
        let mut map: HashMap<String, String> = load_json(&self.path)?;
        match map.get(&entry) {
            None => {
                map.insert(entry.clone(), presented.to_string());
                save_json(&self.path, &map)?;
                eprintln!("SSH host key pinned for {}: {}", entry, presented);
                Ok(true)
            }
            Some(pinned) if pinned == presented => Ok(true),
            Some(pinned) => {
                eprintln!(
                    "SSH host key mismatch for {}: expected {}, presented {}",
                    entry, pinned, presented
                );
                Ok(false)
            }
        }
    }
}

/// Port of Kotlin `SshHostKeyStore.TofuVerifier`.
#[derive(Debug, Clone)]
pub struct TofuVerifier {
    store: SshHostKeyStore,
}

impl TofuVerifier {
    pub fn new(store: SshHostKeyStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &SshHostKeyStore {
        &self.store
    }

    pub fn verify(&self, hostname: &str, port: u16, key_blob: &[u8]) -> anyhow::Result<bool> {
        self.store.verify(hostname, port, key_blob)
    }

    pub fn verify_fingerprint(&self, hostname: &str, port: u16, presented: &str) -> anyhow::Result<bool> {
        self.store.verify_fingerprint(hostname, port, presented)
    }

    pub fn find_existing_algorithms(&self, _hostname: &str, _port: u16) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_key_normalization() {
        assert_eq!(entry_key("Example.COM ", 22), "example.com:22");
        assert_eq!(entry_key(" 192.168.1.1 ", 2222), "192.168.1.1:2222");
    }

    #[test]
    fn test_sha256_known_vectors() {
        // Empty string
        let digest_empty = sha256(b"");
        let hex_empty = crate::store::secrets::to_hex(&digest_empty);
        assert_eq!(
            hex_empty,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // "abc"
        let digest_abc = sha256(b"abc");
        let hex_abc = crate::store::secrets::to_hex(&digest_abc);
        assert_eq!(
            hex_abc,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_base64_encode_unpadded() {
        assert_eq!(base64_encode_unpadded(b""), "");
        assert_eq!(base64_encode_unpadded(b"f"), "Zg");
        assert_eq!(base64_encode_unpadded(b"fo"), "Zm8");
        assert_eq!(base64_encode_unpadded(b"foo"), "Zm9v");
        assert_eq!(base64_encode_unpadded(b"foob"), "Zm9vYg");
        assert_eq!(base64_encode_unpadded(b"fooba"), "Zm9vYmE");
        assert_eq!(base64_encode_unpadded(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_fingerprint_computation() {
        assert_eq!(
            fingerprint(b""),
            "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU"
        );
        assert_eq!(
            fingerprint(b"abc"),
            "SHA256:ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0"
        );
    }

    #[test]
    fn test_tofu_pin_match_and_mismatch() {
        let tmp = std::env::temp_dir().join(format!("csqtt_host_keys_test_{}.json", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let store = SshHostKeyStore::new(&tmp);
        let key1 = b"ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC...";
        let key2 = b"ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQD...";

        // First verification pins the key
        let ok1 = store.verify("server.net", 22, key1).unwrap();
        assert!(ok1);

        // Pinned key matches
        let ok2 = store.verify("SERVER.NET ", 22, key1).unwrap();
        assert!(ok2);

        // Different key mismatches
        let ok3 = store.verify("server.net", 22, key2).unwrap();
        assert!(!ok3);

        // Clear removes pinned key
        store.clear("server.net", 22).unwrap();
        assert_eq!(store.get("server.net", 22).unwrap(), None);

        // Re-pinning key2 now succeeds
        let ok4 = store.verify("server.net", 22, key2).unwrap();
        assert!(ok4);

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_tofu_verifier_wrapper() {
        let tmp = std::env::temp_dir().join(format!("csqtt_tofu_verifier_{}.json", std::process::id()));
        let store = SshHostKeyStore::new(&tmp);
        let verifier = TofuVerifier::new(store);

        let key = b"ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTY...";
        assert!(verifier.verify("host1", 22, key).unwrap());
        assert!(verifier.verify("host1", 22, key).unwrap());
        assert_eq!(verifier.find_existing_algorithms("host1", 22), Vec::<String>::new());

        let _ = std::fs::remove_file(&tmp);
    }
}

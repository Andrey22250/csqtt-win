//! Port of TunnelSessionId.kt.

use rand::Rng;

/// Generates a new 32-character hexadecimal tunnel session id using random bytes.
pub fn new_tunnel_session_id() -> String {
    new_tunnel_session_id_with(|bytes| {
        rand::rng().fill_bytes(bytes);
    })
}

/// Generates a new 32-character hexadecimal tunnel session id using a custom byte filler function.
pub fn new_tunnel_session_id_with<F>(fill_bytes: F) -> String
where
    F: FnOnce(&mut [u8]),
{
    let mut bytes = [0u8; 16];
    fill_bytes(&mut bytes);
    let mut out = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_encodes_all_sixteen_random_bytes() {
        let value = new_tunnel_session_id_with(|bytes| {
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = index as u8;
            }
        });

        assert_eq!("000102030405060708090a0b0c0d0e0f", value);
        assert_eq!(32, value.len());
    }

    #[test]
    fn session_id_uses_lowercase_hexadecimal_alphabet() {
        let value = new_tunnel_session_id_with(|bytes| bytes.fill(0xff));

        assert_eq!(32, value.len());
        assert!(value.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
    }

    #[test]
    fn separate_random_inputs_result_in_separate_session_ids() {
        let first = new_tunnel_session_id_with(|bytes| bytes.fill(0x11));
        let second = new_tunnel_session_id_with(|bytes| bytes.fill(0x22));

        assert_ne!(first, second);
    }
}

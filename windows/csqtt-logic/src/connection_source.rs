//! Port of ConnectionSource.kt.

/// Pure connection credentials and endpoint details.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ConnectionSource {
    pub peer: String,
    pub password: String,
    pub hashes: String,
    pub hashes_from_link: bool,
    pub vk_access_token: String,
    pub token_from_link: bool,
}

impl ConnectionSource {
    pub fn new(
        peer: impl Into<String>,
        password: impl Into<String>,
        hashes: impl Into<String>,
        hashes_from_link: bool,
        vk_access_token: impl Into<String>,
        token_from_link: bool,
    ) -> Self {
        Self {
            peer: peer.into(),
            password: password.into(),
            hashes: hashes.into(),
            hashes_from_link,
            vk_access_token: vk_access_token.into(),
            token_from_link,
        }
    }

    /// Appends the port to base peer address if not already present.
    pub fn format_peer(base_peer: &str, server_peer_port: i32) -> String {
        if base_peer.contains(':') {
            base_peer.to_string()
        } else {
            format!("{base_peer}:{server_peer_port}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_source_construction_and_equality() {
        let cs1 = ConnectionSource::new(
            "1.2.3.4:46000",
            "secret",
            "hash1,hash2",
            true,
            "token123",
            false,
        );
        let cs2 = ConnectionSource {
            peer: "1.2.3.4:46000".to_string(),
            password: "secret".to_string(),
            hashes: "hash1,hash2".to_string(),
            hashes_from_link: true,
            vk_access_token: "token123".to_string(),
            token_from_link: false,
        };

        assert_eq!(cs1, cs2);
        assert_eq!(cs1.peer, "1.2.3.4:46000");
        assert_eq!(cs1.password, "secret");
        assert_eq!(cs1.hashes, "hash1,hash2");
        assert!(cs1.hashes_from_link);
        assert_eq!(cs1.vk_access_token, "token123");
        assert!(!cs1.token_from_link);
    }

    #[test]
    fn format_peer_handles_ports() {
        assert_eq!(ConnectionSource::format_peer("1.2.3.4", 46000), "1.2.3.4:46000");
        assert_eq!(ConnectionSource::format_peer("1.2.3.4:5555", 46000), "1.2.3.4:5555");
        assert_eq!(ConnectionSource::format_peer("[::1]", 46000), "[::1]"); // Kotlin: contains(':') -> unchanged
        assert_eq!(ConnectionSource::format_peer("[::1]:9999", 46000), "[::1]:9999");
    }

    #[test]
    fn connection_source_serde_roundtrip() {
        let cs = ConnectionSource::new("peer", "pwd", "h1", false, "tok", true);
        let json = serde_json::to_string(&cs).expect("serialize");
        let deserialized: ConnectionSource = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cs, deserialized);
    }
}

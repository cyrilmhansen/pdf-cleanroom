/// Mask module — replaces detected secrets with various masked representations.

use sha2::{Digest, Sha256};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaskMode {
    /// Replace with full-width black blocks (████████)
    BlackBlock,
    /// Replace with a label like [SUPPRIMÉ: EMAIL]
    Label,
    /// Replace with same-width black blocks
    SameWidth,
    /// Replace with a discreet marker (⟦ supprimé ⟧)
    ThinAir,
    /// Replace with a short hash reference [SUPPRIMÉ#8F3A]
    Hash,
}

impl MaskMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "black-block" => Some(Self::BlackBlock),
            "label" => Some(Self::Label),
            "same-width" => Some(Self::SameWidth),
            "thin-air" => Some(Self::ThinAir),
            "hash" => Some(Self::Hash),
            _ => None,
        }
    }


}

/// Apply the chosen mask mode to a secret value.
pub fn apply_mask(value: &str, kind: &str, mode: MaskMode) -> String {
    match mode {
        MaskMode::BlackBlock => {
            // At least 4 blocks, more for longer secrets
            let count = value.chars().count().max(4);
            "█".repeat(count)
        }
        MaskMode::Label => {
            let label = match kind {
                "email" => "EMAIL",
                "phone_fr" => "TÉLÉPHONE",
                "iban" => "IBAN",
                other => other,
            };
            format!("[SUPPRIMÉ: {label}]")
        }
        MaskMode::SameWidth => {
            let count = value.chars().count().max(1);
            "█".repeat(count)
        }
        MaskMode::ThinAir => "⟦ supprimé ⟧".to_string(),
        MaskMode::Hash => {
            let mut hasher = Sha256::new();
            hasher.update(value.as_bytes());
            let result = hasher.finalize();
            let first_bytes = &result[..2];
            let mut hex = String::with_capacity(4);
            write!(hex, "{:04x}", u16::from_be_bytes([first_bytes[0], first_bytes[1]])).unwrap();
            let short = &hex[..4];
            format!("[SUPPRIMÉ#{short}]")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_black_block() {
        let r = apply_mask("test@example.com", "email", MaskMode::BlackBlock);
        assert_eq!(r.chars().count(), 16); // "test@example.com" is 16 chars
        assert!(r.chars().all(|c| c == '█'));
    }

    #[test]
    fn test_black_block_short() {
        let r = apply_mask("a", "email", MaskMode::BlackBlock);
        assert_eq!(r.chars().count(), 4); // minimum 4
    }

    #[test]
    fn test_label_email() {
        let r = apply_mask("user@x.com", "email", MaskMode::Label);
        assert_eq!(r, "[SUPPRIMÉ: EMAIL]");
    }

    #[test]
    fn test_label_phone() {
        let r = apply_mask("0123456789", "phone_fr", MaskMode::Label);
        assert_eq!(r, "[SUPPRIMÉ: TÉLÉPHONE]");
    }

    #[test]
    fn test_label_iban() {
        let r = apply_mask("FR7630006000011234567890189", "iban", MaskMode::Label);
        assert_eq!(r, "[SUPPRIMÉ: IBAN]");
    }

    #[test]
    fn test_same_width() {
        let r = apply_mask("12345", "email", MaskMode::SameWidth);
        assert_eq!(r.chars().count(), 5);
        assert!(r.chars().all(|c| c == '█'));
    }

    #[test]
    fn test_thin_air() {
        let r = apply_mask("anything", "email", MaskMode::ThinAir);
        assert_eq!(r, "⟦ supprimé ⟧");
    }

    #[test]
    fn test_hash() {
        let r = apply_mask("hello", "email", MaskMode::Hash);
        assert!(r.starts_with("[SUPPRIMÉ#"));
        assert!(r.ends_with(']'));
        // [SUPPRIMÉ#XXXX] = 15 characters (É multi-byte, so check char count)
        assert_eq!(r.chars().count(), 15);
    }

    #[test]
    fn test_consistent_hash() {
        let a = apply_mask("secret@email.com", "email", MaskMode::Hash);
        let b = apply_mask("secret@email.com", "email", MaskMode::Hash);
        assert_eq!(a, b);
    }

    #[test]
    fn test_from_str() {
        assert_eq!(MaskMode::from_str("black-block"), Some(MaskMode::BlackBlock));
        assert_eq!(MaskMode::from_str("label"), Some(MaskMode::Label));
        assert_eq!(MaskMode::from_str("hash"), Some(MaskMode::Hash));
        assert_eq!(MaskMode::from_str("invalid"), None);
    }
}

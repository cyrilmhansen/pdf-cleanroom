/// Detection module — regex-based secret detection.

use regex::Regex;

/// A single detection result with positions relative to the input text.
#[derive(Debug, Clone)]
pub struct Detection {
    pub kind: String,
    pub value: String,
    pub start: usize,
    pub end: usize,
}

/// Pre-compiled set of detection patterns.
pub struct Detector {
    email_re: Regex,
    phone_fr_re: Regex,
    iban_re: Regex,
}

impl Detector {
    /// Compile all detection regexes.
    /// Panics only if static regexes are invalid (should never happen).
    pub fn new() -> Self {
        // Email: basic RFC-compatible pattern
        let email_re =
            Regex::new(r"[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*")
                .expect("invalid email regex");

        // French phone: +33, 0X XX XX XX XX, various separators
        // Matches 0X XX XX XX XX, +33 X XX XX XX XX, 0033 X XX XX XX XX
        let phone_fr_re = Regex::new(
            r"(?:(?:[+]|00)33[\s.-]?|\b0)[1-9](?:[\s.-]?(?:\d{2})){4}\b"
        ).expect("invalid phone_fr regex");

        // IBAN (French): FRXX XXXX XXXX XXXX XXXX XXXX XXX
        // Also supports other countries for completeness
        let iban_re = Regex::new(
            r"\b[A-Z]{2}\d{2}(?:[ ]?\d{4}){4,7}(?:[ ]?\d{1,3})?\b"
        ).expect("invalid iban regex");

        Self {
            email_re,
            phone_fr_re,
            iban_re,
        }
    }

    /// Scan a single text string for all patterns.
    pub fn scan_text(&self, text: &str) -> Vec<Detection> {
        let mut results = Vec::new();

        // Email detection
        for m in self.email_re.find_iter(text) {
            results.push(Detection {
                kind: "email".into(),
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            });
        }

        // French phone detection
        for m in self.phone_fr_re.find_iter(text) {
            results.push(Detection {
                kind: "phone_fr".into(),
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            });
        }

        // IBAN detection
        for m in self.iban_re.find_iter(text) {
            results.push(Detection {
                kind: "iban".into(),
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            });
        }

        results
    }
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> Detector {
        Detector::new()
    }

    #[test]
    fn test_detect_email_simple() {
        let d = detector();
        let results = d.scan_text("Contact: user@example.com");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "email");
        assert_eq!(results[0].value, "user@example.com");
    }

    #[test]
    fn test_detect_email_multiple() {
        let d = detector();
        let results = d.scan_text("a@b.com c@d.org");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_detect_no_false_email() {
        let d = detector();
        let results = d.scan_text("just text without emails");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_detect_email_subdomain() {
        let d = detector();
        let results = d.scan_text("user@sub.example.co.uk");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, "user@sub.example.co.uk");
    }

    #[test]
    fn test_detect_phone_french_standard() {
        let d = detector();
        let results = d.scan_text("appelez au 06 11 22 33 44");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "phone_fr");
        assert_eq!(results[0].value, "06 11 22 33 44");
    }

    #[test]
    fn test_detect_phone_french_dots() {
        let d = detector();
        let results = d.scan_text("06.11.22.33.44");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_detect_phone_french_dashes() {
        let d = detector();
        let results = d.scan_text("06-11-22-33-44");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_detect_phone_french_international() {
        let d = detector();
        let results = d.scan_text("+33 6 11 22 33 44");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, "+33 6 11 22 33 44");
    }

    #[test]
    fn test_detect_phone_french_leading_zero() {
        let d = detector();
        let results = d.scan_text("0123456789");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, "0123456789");
    }

    #[test]
    fn test_detect_iban_french() {
        let d = detector();
        let results = d.scan_text("FR76 3000 6000 0112 3456 7890 189");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "iban");
    }

    #[test]
    fn test_detect_iban_no_spaces() {
        let d = detector();
        let results = d.scan_text("FR7630006000011234567890189");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_detect_iban_german() {
        let d = detector();
        let results = d.scan_text("DE89370400440532013000");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "iban");
    }

    #[test]
    fn test_detect_iban_short_fragment_no_false() {
        let d = detector();
        // "FR76" alone is too short (only 4 chars) for the IBAN pattern
        let results = d.scan_text("le code FR76 n'est pas un IBAN complet");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_scan_text_multiple_kinds() {
        let d = detector();
        let results = d.scan_text(
            "Email: jean.dupont@example.com\n\
             Tél: 06 11 22 33 44\n\
             IBAN: FR7630006000011234567890189",
        );
        assert_eq!(results.len(), 3);
        let kinds: Vec<&str> = results.iter().map(|r| r.kind.as_str()).collect();
        assert!(kinds.contains(&"email"));
        assert!(kinds.contains(&"phone_fr"));
        assert!(kinds.contains(&"iban"));
    }

    #[test]
    fn test_detect_phone_not_email() {
        // Phone number should not match email pattern
        let d = detector();
        let text = "06 11 22 33 44";
        let email_results: Vec<_> = d
            .scan_text(text)
            .into_iter()
            .filter(|r| r.kind == "email")
            .collect();
        assert_eq!(email_results.len(), 0);
    }
}

//! Small format validators shared by several DQ checks.
//!
//! These helpers do not validate against authoritative registries
//! (the GLEIF LEI registry, the ISO 4217 currency list). They check
//! the syntactic shape only — enough to catch obvious typos and
//! detect garbage fields cheaply.

/// Returns true if `s` matches the ISO 17442 LEI shape: exactly 20
/// characters, the first 18 alphanumeric (A-Z, 0-9) and the last 2
/// numeric.
///
/// The ISO 17442 check-digit verification is **not** performed: it is
/// already enforced by every Trade Repository, and re-implementing it
/// here adds little value for an MVP scanner.
pub fn is_valid_lei(s: &str) -> bool {
    if s.len() != 20 {
        return false;
    }
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i < 18 {
            if !(b.is_ascii_uppercase() || b.is_ascii_digit()) {
                return false;
            }
        } else if !b.is_ascii_digit() {
            return false;
        }
    }
    true
}

/// Returns true if `s` is a syntactically valid ISO 4217 currency
/// code: exactly three uppercase ASCII letters.
///
/// The currency code list (`EUR`, `USD`, ...) is **not** validated;
/// only the shape is. Catching unknown codes is left to a future
/// rule pack that ships the official list.
pub fn is_valid_currency_code(s: &str) -> bool {
    if s.len() != 3 {
        return false;
    }
    s.bytes().all(|b| b.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_lei_passes() {
        assert!(is_valid_lei("ABCDEFGHIJKLMNOPQR01"));
        assert!(is_valid_lei("12345678901234567890"));
        // ends with a letter → invalid even though length is 20
        assert!(!is_valid_lei("DUMMYCPTY1000000000A"));
    }

    #[test]
    fn wrong_length_fails() {
        assert!(!is_valid_lei("ABC"));
        assert!(!is_valid_lei("ABCDEFGHIJKLMNOPQR0")); // 19
        assert!(!is_valid_lei("ABCDEFGHIJKLMNOPQR012")); // 21
        assert!(!is_valid_lei(""));
    }

    #[test]
    fn lowercase_fails() {
        assert!(!is_valid_lei("abcdefghijklmnopqr01"));
    }

    #[test]
    fn non_digit_check_suffix_fails() {
        assert!(!is_valid_lei("ABCDEFGHIJKLMNOPQRZZ"));
        assert!(!is_valid_lei("ABCDEFGHIJKLMNOPQR0A"));
    }

    #[test]
    fn valid_currency_passes() {
        assert!(is_valid_currency_code("EUR"));
        assert!(is_valid_currency_code("USD"));
        assert!(is_valid_currency_code("XAU"));
    }

    #[test]
    fn invalid_currency_fails() {
        assert!(!is_valid_currency_code("EU"));
        assert!(!is_valid_currency_code("EURO"));
        assert!(!is_valid_currency_code("eur"));
        assert!(!is_valid_currency_code("E0R"));
        assert!(!is_valid_currency_code(""));
    }
}

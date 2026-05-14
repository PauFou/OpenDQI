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

/// Returns true if `value` matches any entry in `allowed`, comparing
/// ASCII case-insensitively. Useful for enumerated-string field
/// validation against a small fixed set.
pub fn is_in(value: &str, allowed: &[&'static str]) -> bool {
    allowed.iter().any(|a| a.eq_ignore_ascii_case(value))
}

/// Returns true if a decimal fits within the given integer-digit and
/// fractional-digit bounds. ESMA monetary amounts are typically
/// `decimal:18.5` (max 18 integer digits + 5 fractional digits).
pub fn within_decimal_bounds(d: &rust_decimal::Decimal, max_int: u32, max_scale: u32) -> bool {
    if d.scale() > max_scale {
        return false;
    }
    // Integer-digit count = digits of |d.trunc()|. Use absolute value
    // to handle negatives uniformly.
    let abs = d.abs();
    let trunc = abs.trunc();
    if trunc.is_zero() {
        return true;
    }
    let s = trunc.to_string();
    // s is like "12345" (no fractional part because trunc).
    s.len() as u32 <= max_int
}

/// Currency → maximum decimal places per ISO 4217 (sample of the
/// most widely used codes plus a few illustrative non-ISO codes for
/// crypto and precious metals). When a currency is not listed,
/// `currency_max_scale` returns `None` and currency-specific checks
/// fall back to the generic `decimal:18.5` precision.
pub const CURRENCY_DECIMALS: &[(&str, u32)] = &[
    // Common 2-decimal currencies
    ("USD", 2),
    ("EUR", 2),
    ("GBP", 2),
    ("CHF", 2),
    ("CAD", 2),
    ("AUD", 2),
    ("NZD", 2),
    ("SEK", 2),
    ("NOK", 2),
    ("DKK", 2),
    ("PLN", 2),
    ("CZK", 2),
    ("HUF", 2),
    ("ZAR", 2),
    ("HKD", 2),
    ("SGD", 2),
    ("CNY", 2),
    ("INR", 2),
    ("MXN", 2),
    ("BRL", 2),
    // Zero-decimal currencies (no minor unit)
    ("JPY", 0),
    ("KRW", 0),
    ("CLP", 0),
    ("ISK", 0),
    ("VND", 0),
    // Three-decimal currencies
    ("BHD", 3),
    ("JOD", 3),
    ("KWD", 3),
    ("OMR", 3),
    ("TND", 3),
    // Four-decimal currencies
    ("CLF", 4),
    ("UYW", 4),
    // Crypto / precious metals (illustrative, not ISO 4217)
    ("BTC", 8),
    ("ETH", 18),
    ("XAU", 6),
    ("XAG", 6),
];

/// Returns the maximum number of fractional digits expected for an
/// amount in the given currency code (case-insensitive). Returns
/// `None` when the currency is not in [`CURRENCY_DECIMALS`].
pub fn currency_max_scale(ccy: &str) -> Option<u32> {
    let ccy = ccy.trim();
    CURRENCY_DECIMALS
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(ccy))
        .map(|(_, scale)| *scale)
}

/// Canonicalises an `asset_class` string to one of OpenDQI's short
/// codes — `IR`, `CR`, `EQ`, `FX`, `CO` — or returns `None` when the
/// input does not match any known alias. The mapping covers the ESMA
/// short codes and the variants used by ISO 20022 (`INTR`, `CRDT`,
/// `EQTY`, `CURR`, `COMM`).
///
/// Centralises the lookup so per-asset-class threshold checks
/// (`EMIR.RMT.NFC_ABOVE_CLEARING_THRESHOLD`, …) don't repeat the
/// matching inline.
pub fn canonical_asset_class(raw: &str) -> Option<&'static str> {
    match raw.trim().to_uppercase().as_str() {
        "IR" | "INTR" | "RATE" => Some("IR"),
        "CR" | "CRDT" => Some("CR"),
        "EQ" | "EQTY" => Some("EQ"),
        "FX" | "CU" | "CURR" => Some("FX"),
        "CO" | "COMM" => Some("CO"),
        _ => None,
    }
}

/// Returns true if `s` matches the ISO 6166 ISIN shape: 12 characters
/// total — 2 uppercase letters (country code), 9 alphanumeric, and 1
/// numeric check digit.
///
/// The ISIN Luhn-variant check-digit is **not** verified for MVP — it
/// is enforced upstream by every reputable security identifier
/// provider.
pub fn is_valid_isin(s: &str) -> bool {
    if s.len() != 12 {
        return false;
    }
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        match i {
            0 | 1 => {
                if !b.is_ascii_uppercase() {
                    return false;
                }
            }
            2..=10 => {
                if !(b.is_ascii_uppercase() || b.is_ascii_digit()) {
                    return false;
                }
            }
            11 => {
                if !b.is_ascii_digit() {
                    return false;
                }
            }
            _ => unreachable!(),
        }
    }
    true
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

    #[test]
    fn valid_isin_passes() {
        // Real-shaped ISINs (check digits not verified)
        assert!(is_valid_isin("DE0001135275"));
        assert!(is_valid_isin("US1234567890"));
        assert!(is_valid_isin("FR0000120628"));
    }

    #[test]
    fn invalid_isin_fails() {
        assert!(!is_valid_isin("DE000113527")); // 11 chars
        assert!(!is_valid_isin("DE00011352755")); // 13 chars
        assert!(!is_valid_isin("de0001135275")); // lowercase country
        assert!(!is_valid_isin("12DE01135275")); // digits in country
        assert!(!is_valid_isin("DE000113527A")); // letter check digit
        assert!(!is_valid_isin(""));
    }

    #[test]
    fn within_decimal_bounds_accepts_normal_values() {
        use rust_decimal::Decimal;
        use std::str::FromStr;
        assert!(within_decimal_bounds(&Decimal::from(1000000), 18, 5));
        assert!(within_decimal_bounds(
            &Decimal::from_str("123.45").unwrap(),
            18,
            5
        ));
        assert!(within_decimal_bounds(&Decimal::ZERO, 18, 5));
    }

    #[test]
    fn within_decimal_bounds_rejects_too_many_int_digits() {
        use rust_decimal::Decimal;
        use std::str::FromStr;
        // 19 digits before the decimal point.
        let big = Decimal::from_str("1234567890123456789").unwrap();
        assert!(!within_decimal_bounds(&big, 18, 5));
    }

    #[test]
    fn currency_max_scale_known_codes() {
        assert_eq!(currency_max_scale("JPY"), Some(0));
        assert_eq!(currency_max_scale("usd"), Some(2));
        assert_eq!(currency_max_scale("BHD"), Some(3));
        assert_eq!(currency_max_scale("BTC"), Some(8));
    }

    #[test]
    fn currency_max_scale_unknown_returns_none() {
        assert_eq!(currency_max_scale("XYZ"), None);
        assert_eq!(currency_max_scale(""), None);
    }

    #[test]
    fn within_decimal_bounds_rejects_too_much_scale() {
        use rust_decimal::Decimal;
        use std::str::FromStr;
        // 6 fractional digits.
        let d = Decimal::from_str("1.234567").unwrap();
        assert!(!within_decimal_bounds(&d, 18, 5));
    }

    #[test]
    fn canonical_asset_class_recognises_short_codes_and_aliases() {
        // Canonical short codes pass through.
        assert_eq!(canonical_asset_class("IR"), Some("IR"));
        assert_eq!(canonical_asset_class("CR"), Some("CR"));
        assert_eq!(canonical_asset_class("EQ"), Some("EQ"));
        assert_eq!(canonical_asset_class("FX"), Some("FX"));
        assert_eq!(canonical_asset_class("CO"), Some("CO"));
        // ISO 20022 / ESMA aliases.
        assert_eq!(canonical_asset_class("INTR"), Some("IR"));
        assert_eq!(canonical_asset_class("CRDT"), Some("CR"));
        assert_eq!(canonical_asset_class("EQTY"), Some("EQ"));
        assert_eq!(canonical_asset_class("CURR"), Some("FX"));
        assert_eq!(canonical_asset_class("COMM"), Some("CO"));
        // Case + whitespace insensitive.
        assert_eq!(canonical_asset_class("  ir "), Some("IR"));
    }

    #[test]
    fn canonical_asset_class_rejects_unknown() {
        assert_eq!(canonical_asset_class("FOO"), None);
        assert_eq!(canonical_asset_class(""), None);
    }
}

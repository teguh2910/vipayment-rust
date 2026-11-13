use chrono::{Duration, SecondsFormat, Utc};

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn rfc3339_after_seconds(seconds: i64) -> String {
    (Utc::now() + Duration::seconds(seconds)).to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn rfc3339_after_minutes(minutes: i64) -> String {
    rfc3339_after_seconds(minutes * 60)
}

pub fn format_idr(amount: i64) -> String {
    let negative = amount < 0;
    let digits = amount.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push('.');
        }
        out.push(digit);
    }

    if negative {
        format!("-{out}")
    } else {
        out
    }
}

pub fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();

    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

pub fn phone_digits(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Returns the digit forms of a phone number that could match a stored contact
/// (`0812...`, `62812...`, `+62 812-...`).
pub fn phone_variants(value: &str) -> Vec<String> {
    let digits = phone_digits(value);
    let mut variants = vec![digits.clone()];

    if let Some(rest) = digits.strip_prefix("62") {
        if rest.len() >= 8 {
            variants.push(format!("0{rest}"));
        }
    } else if let Some(rest) = digits.strip_prefix('0') {
        if rest.len() >= 8 {
            variants.push(format!("62{rest}"));
        }
    }

    variants.sort();
    variants.dedup();
    variants
}

pub fn mask_contact(contact: &str) -> String {
    let contact = contact.trim();

    if let Some(at) = contact.find('@') {
        let (local, domain) = contact.split_at(at);
        let visible: String = local.chars().take(2).collect();
        return format!("{visible}***{domain}");
    }

    let chars: Vec<char> = contact.chars().collect();
    if chars.len() <= 6 {
        return "****".to_string();
    }

    let head: String = chars.iter().take(4).collect();
    let tail: String = chars.iter().skip(chars.len() - 3).collect();
    format!("{head}****{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_rupiah_with_thousand_separators() {
        assert_eq!(format_idr(0), "0");
        assert_eq!(format_idr(950), "950");
        assert_eq!(format_idr(21_000), "21.000");
        assert_eq!(format_idr(1_250_000), "1.250.000");
    }

    #[test]
    fn masks_phone_numbers_and_emails() {
        assert_eq!(mask_contact("081234567890"), "0812****890");
        assert_eq!(mask_contact("teguh@example.com"), "te***@example.com");
        assert_eq!(mask_contact("12345"), "****");
    }

    #[test]
    fn builds_phone_variants_for_lookup() {
        assert_eq!(
            phone_variants("+62 812-3456-7890"),
            vec!["081234567890".to_string(), "6281234567890".to_string()]
        );
        assert_eq!(
            phone_variants("0812 3456 7890"),
            vec!["081234567890".to_string(), "6281234567890".to_string()]
        );
        assert_eq!(phone_variants("0812"), vec!["0812".to_string()]);
    }

    #[test]
    fn compares_tokens_in_constant_time() {
        assert!(constant_time_eq("secret-token", "secret-token"));
        assert!(!constant_time_eq("secret-token", "secret-toke"));
        assert!(!constant_time_eq("", "x"));
    }
}

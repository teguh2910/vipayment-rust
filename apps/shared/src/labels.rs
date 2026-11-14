/// Human-readable labels for catalog categories. Lives in the shared crate so
/// the order service can render `category_label` without depending on the
/// catalog service.
pub fn category_label(category: &str) -> String {
    match category {
        "pulsa-reguler" => "Pulsa Reguler".to_string(),
        "pulsa-transfer" => "Pulsa Transfer".to_string(),
        "pulsa-internasional" => "Pulsa Internasional".to_string(),
        "paket-internet" => "Paket Internet".to_string(),
        "paket-telepon" => "Paket Telepon".to_string(),
        "paket-lainnya" => "Paket Lainnya".to_string(),
        "saldo-emoney" => "Saldo E-Wallet".to_string(),
        "token-pln" => "Token PLN".to_string(),
        "streaming-tv" => "TV & Streaming".to_string(),
        "voucher-game" => "Voucher".to_string(),
        other => title_case(&other.replace('-', " ")),
    }
}

pub fn title_case(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut capitalize = true;

    for ch in value.chars() {
        if ch.is_whitespace() || ch == '-' || ch == '/' {
            capitalize = true;
            out.push(ch);
        } else if capitalize {
            out.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            out.extend(ch.to_lowercase());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_known_and_unknown_categories() {
        assert_eq!(category_label("token-pln"), "Token PLN");
        assert_eq!(category_label("pulsa-reguler"), "Pulsa Reguler");
        assert_eq!(category_label("some-new-type"), "Some New Type");
    }
}

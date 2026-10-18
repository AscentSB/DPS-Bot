//! String helpers: `cleanName` (src/lib/nbt.js), JS `trim()` whitespace, and `Number(string)`.

use std::borrow::Cow;

/// JS `WhiteSpace` + `LineTerminator` (what `String.prototype.trim` removes). Differs from Rust's `char::is_whitespace`
/// (JS includes U+FEFF, excludes U+0085).
#[inline]
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}'
            | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

/// JS `s.trim()`.
#[inline]
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// Length of a `%%[a-z_]+%%` match (case-insensitive) at the start of `s`, if any.
fn color_code_len(s: &[u8]) -> Option<usize> {
    if s.len() < 5 || s[0] != b'%' || s[1] != b'%' {
        return None;
    }
    let mut i = 2;
    while i < s.len() && (s[i].is_ascii_alphabetic() || s[i] == b'_') {
        i += 1;
    }
    if i > 2 && i + 1 < s.len() && s[i] == b'%' && s[i + 1] == b'%' {
        Some(i + 2)
    } else {
        None
    }
}

/// JS `cleanName(name)`: `String(name ?? '').replace(/%%[a-z_]+%%/gi, '').replace(/§./g, '').trim()`.
/// Borrows when nothing changes.
pub fn clean_name_cow(name: &str) -> Cow<'_, str> {
    if !name.contains('%') && !name.contains('§') {
        return Cow::Borrowed(js_trim(name));
    }
    // Pass 1: %%color%% codes.
    let bytes = name.as_bytes();
    let mut first = String::with_capacity(name.len());
    let mut i = 0;
    while i < bytes.len() {
        if let Some(n) = color_code_len(&bytes[i..]) {
            i += n;
            continue;
        }
        let c = name[i..].chars().next().expect("char boundary");
        first.push(c);
        i += c.len_utf8();
    }
    // Pass 2: § plus the next character (any but a line terminator).
    let mut out = String::with_capacity(first.len());
    let mut chars = first.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§' {
            if let Some(&n) = chars.peek() {
                if !matches!(n, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                    chars.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    let trimmed = js_trim(&out);
    if trimmed.len() == out.len() {
        Cow::Owned(out)
    } else {
        Cow::Owned(trimmed.to_string())
    }
}

/// JS `cleanName(name)`.
pub fn clean_name(name: &str) -> String {
    clean_name_cow(name).into_owned()
}

/// JS `Number(s)` for a string: trims JS whitespace; `""` is 0; decimal literals, `Infinity`, and `0x` / `0o` / `0b`
/// integers; anything else is NaN.
pub fn js_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let radix = |p: &str, r: u32| -> Option<f64> {
        let digits = t.strip_prefix(p).or_else(|| t.strip_prefix(&p.to_ascii_uppercase()))?;
        if digits.is_empty() {
            return Some(f64::NAN);
        }
        let mut v = 0.0f64;
        for c in digits.chars() {
            v = v * r as f64 + c.to_digit(r)? as f64;
        }
        Some(v)
    };
    if let Some(v) = radix("0x", 16).or_else(|| radix("0o", 8)).or_else(|| radix("0b", 2)) {
        return v;
    }
    if t.starts_with("0x") || t.starts_with("0X") || t.starts_with("0o") || t.starts_with("0O") || t.starts_with("0b") || t.starts_with("0B") {
        return f64::NAN;
    }
    // Decimal literal: digits, '.', exponent and a leading sign only (Rust's parser also takes "inf" / "nan").
    if !t.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-')) {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_names() {
        assert_eq!(clean_name("%%light_purple%%Hyperion"), "Hyperion");
        assert_eq!(clean_name("§dHyperion §r"), "Hyperion");
        assert_eq!(clean_name("  Plain  "), "Plain");
        assert_eq!(clean_name("%%%abc%%x"), "%x");
        assert_eq!(clean_name("%%A_B%%y%%z"), "y%%z");
        assert_eq!(clean_name("§"), "§");
        assert_eq!(clean_name("a§\nb"), "a§\nb");
        assert_eq!(clean_name("\u{FEFF}x\u{85}"), "x\u{85}");
    }

    #[test]
    fn numbers() {
        assert_eq!(js_number("101"), 101.0);
        assert_eq!(js_number(" 2.5 "), 2.5);
        assert_eq!(js_number(""), 0.0);
        assert!(js_number("1a").is_nan());
        assert!(js_number("inf").is_nan());
        assert_eq!(js_number("0x10"), 16.0);
        assert_eq!(js_number("-Infinity"), f64::NEG_INFINITY);
    }
}

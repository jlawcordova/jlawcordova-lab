//! JavaScript number semantics (docs/intents/2026-10-rust-engine/spec.md, D2,
//! "Byte parity"). The Node engine printed and rounded numbers the JavaScript
//! way, and every SVG, CSS rule and JSON result depends on it, so the port
//! uses these instead of Rust's own:
//!
//! - [`round`] is `Math.round`: halves go up (−2.5 → −2), where `f64::round`
//!   goes away from zero (−3).
//! - [`to_fixed`] is `Number.prototype.toFixed`: the exact value, and on a tie
//!   the larger neighbor (of the magnitude), where Rust's `{:.N}` goes to even.
//! - [`to_string`] is `Number.prototype.toString` (and how `JSON.stringify`
//!   and template strings print a number): the shortest digits that read back
//!   to the same double, laid out with JavaScript's exponent rules.
//! - [`num`] makes a JSON value from a double the way JavaScript would print
//!   it: a whole number is an integer.

use serde_json::{Number, Value};

/// `Math.round(x)`: the nearest integer, a half going toward +∞.
pub fn round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let floor = x.floor();
    let r = if x - floor >= 0.5 { floor + 1.0 } else { floor };
    // Math.round keeps the sign of a zero result: Math.round(-0.4) is -0.
    if r == 0.0 && (x < 0.0 || (x == 0.0 && x.is_sign_negative())) {
        -0.0
    } else {
        r
    }
}

/// The shortest decimal digits of `x` (finite, above 0) that read back to `x`,
/// and the exponent `n` such that `x = 0.d1d2… × 10^n`.
fn shortest(x: f64) -> (String, i32) {
    // Rust's `{:e}` prints the shortest round-trip digits: "d.ddde±x".
    let text = format!("{x:e}");
    let (mantissa, exp) = text.split_once('e').expect("{:e} has an exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exp: i32 = exp.parse().expect("{:e}'s exponent is a number");
    (digits, exp + 1)
}

/// `Number.prototype.toString()` with radix 10, as ECMA-262's
/// Number::toString lays it out.
pub fn to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x < 0.0 {
        return format!("-{}", to_string(-x));
    }
    let (digits, n) = shortest(x);
    let k = digits.len() as i32;
    if k <= n && n <= 21 {
        let mut out = digits;
        out.extend(std::iter::repeat_n('0', (n - k) as usize));
        return out;
    }
    if 0 < n && n <= 21 {
        let (int, frac) = digits.split_at(n as usize);
        return format!("{int}.{frac}");
    }
    if -6 < n && n <= 0 {
        return format!("0.{}{digits}", "0".repeat((-n) as usize));
    }
    let e = n - 1;
    let sign = if e >= 0 { '+' } else { '-' };
    let (first, rest) = digits.split_at(1);
    if rest.is_empty() {
        format!("{first}e{sign}{}", e.abs())
    } else {
        format!("{first}.{rest}e{sign}{}", e.abs())
    }
}

/// An integer as JavaScript prints it.
pub fn int_string(n: i64) -> String {
    n.to_string()
}

/// `Number.prototype.toFixed(digits)` (0 ≤ digits ≤ 100).
pub fn to_fixed(x: f64, digits: usize) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.abs() >= 1e21 {
        return to_string(x);
    }
    // -0 prints without a sign, as toFixed does (only x < 0 gets one).
    let negative = x < 0.0;
    let a = x.abs();
    // The exact decimal expansion of the double: a double has at most 1074
    // fractional binary digits, so at most 1074 fractional decimal ones.
    let exact = format!("{a:.1100}");
    let (int, frac) = exact.split_once('.').expect("{:.1100} has a point");
    let kept = &frac[..digits];
    let rest = &frac[digits..];
    // Round half up on the magnitude: n/10^f closest to x, the larger n on a tie.
    let up = match rest.as_bytes().first() {
        Some(b) if *b > b'5' => true,
        Some(b'5') => true,
        _ => false,
    };
    let mut number: Vec<u8> = int.bytes().chain(kept.bytes()).collect();
    if up {
        let mut i = number.len();
        loop {
            if i == 0 {
                number.insert(0, b'1');
                break;
            }
            i -= 1;
            if number[i] == b'9' {
                number[i] = b'0';
            } else {
                number[i] += 1;
                break;
            }
        }
    }
    let text = String::from_utf8(number).expect("digits are ASCII");
    let int_len = text.len() - digits;
    let (i, f) = text.split_at(int_len);
    let i = i.trim_start_matches('0');
    let i = if i.is_empty() { "0" } else { i };
    let body = if digits == 0 { i.to_string() } else { format!("{i}.{f}") };
    if negative {
        format!("-{body}")
    } else {
        body
    }
}

/// `+x.toFixed(digits)`: rounded the way toFixed rounds, back as a number.
pub fn fixed(x: f64, digits: usize) -> f64 {
    to_fixed(x, digits).parse().unwrap_or(f64::NAN)
}

/// `Number.isInteger(x)`.
pub fn is_integer(x: f64) -> bool {
    x.is_finite() && x.fract() == 0.0
}

/// A double as a JSON value, as `JSON.stringify` would write it: a whole
/// number that fits in an i64 is an integer (so `3.0` prints `3`), anything
/// else finite is a float, and NaN and the infinities are null.
pub fn num(x: f64) -> Value {
    if !x.is_finite() {
        return Value::Null;
    }
    if x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        return Value::Number(Number::from(x as i64));
    }
    Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// A JSON number as a double, the way JavaScript holds every number.
pub fn as_f64(v: &Value) -> Option<f64> {
    v.as_f64()
}

/// A JSON number as JavaScript prints it.
pub fn number_string(n: &Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        // Past 2^53 JavaScript would have rounded it; print it as a double.
        return to_string(u as f64);
    }
    to_string(n.as_f64().unwrap_or(f64::NAN))
}

/// `Number(text)` for a string: trimmed, "" is 0, decimal (with an optional
/// sign, point and exponent), `0x`/`0o`/`0b` integers, and `Infinity`;
/// anything else is NaN.
pub fn parse(text: &str) -> f64 {
    let t = text.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if t.is_empty() {
        return 0.0;
    }
    let (sign, body) = match t.as_bytes()[0] {
        b'+' => (1.0, &t[1..]),
        b'-' => (-1.0, &t[1..]),
        _ => (1.0, t),
    };
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = t.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
                return f64::NAN;
            }
            return digits.chars().fold(0.0, |n, c| n * radix as f64 + c.to_digit(radix).unwrap() as f64);
        }
    }
    let bytes = body.as_bytes();
    let mut i = 0;
    let int_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - int_start;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let f = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - f;
    }
    if digits == 0 {
        return f64::NAN;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let e = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == e {
            return f64::NAN;
        }
    }
    if i != bytes.len() {
        return f64::NAN;
    }
    sign * body.parse::<f64>().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_halves_up() {
        assert_eq!(round(2.5), 3.0);
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round(-0.5), 0.0);
        assert!(round(-0.5).is_sign_negative());
        assert_eq!(round(0.49999999999999994), 0.0);
        assert_eq!(round(1.4999999999999998), 1.0);
    }

    #[test]
    fn prints_like_javascript() {
        let cases = [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (1e-7, "1e-7"),
            (1.5e-7, "1.5e-7"),
            (0.000001, "0.000001"),
            (123456789.125, "123456789.125"),
            (f64::MAX, "1.7976931348623157e+308"),
            (5e-324, "5e-324"),
        ];
        for (x, want) in cases {
            assert_eq!(to_string(x), want, "{x:?}");
        }
    }

    #[test]
    fn fixes_like_javascript() {
        assert_eq!(to_fixed(0.5, 0), "1");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(-2.5, 0), "-3");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(1.45, 1), "1.4");
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(-0.0, 2), "0.00");
        assert_eq!(to_fixed(-0.0001, 2), "-0.00");
        assert_eq!(to_fixed(99.9999, 3), "100.000");
        assert_eq!(to_fixed(1e21, 2), "1e+21");
        assert_eq!(to_fixed(12.3456, 1), "12.3");
    }

    #[test]
    fn parses_like_number() {
        assert_eq!(parse(" 12 "), 12.0);
        assert_eq!(parse(""), 0.0);
        assert_eq!(parse("01"), 1.0);
        assert_eq!(parse("1e3"), 1000.0);
        assert_eq!(parse(".5"), 0.5);
        assert_eq!(parse("5."), 5.0);
        assert_eq!(parse("0x1F"), 31.0);
        assert!(parse("1a").is_nan());
        assert!(parse("inf").is_nan());
        assert_eq!(parse("-Infinity"), f64::NEG_INFINITY);
    }

    #[test]
    fn makes_json_numbers() {
        assert_eq!(num(3.0).to_string(), "3");
        assert_eq!(num(-0.0).to_string(), "0");
        assert_eq!(num(2.5).to_string(), "2.5");
        assert_eq!(num(f64::NAN), Value::Null);
    }
}

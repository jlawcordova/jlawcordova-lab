//! JavaScript's number rules (plan step 3), against the cases
//! `node scripts/goldens.mjs --jsnum` collected from Node: Math.round,
//! toFixed(0–4) and String(x) of every number the corpus's motion and sizes
//! print, and the edge cases.

use engine::format::parse;
use engine::jsnum;

#[test]
fn jsnum_matches_node_on_every_case() {
    let data = parse(include_str!("data/jsnum.json")).unwrap();
    let cases = data["cases"].as_array().unwrap();
    assert!(cases.len() > 1000);
    let mut failures = Vec::new();
    for case in cases {
        let bits = u64::from_str_radix(case["bits"].as_str().unwrap(), 16).unwrap();
        // Node wrote the bytes little-endian.
        let x = f64::from_bits(u64::from_be_bytes(bits.to_le_bytes()));
        let want = case["string"].as_str().unwrap();
        if jsnum::to_string(x) != want {
            failures.push(format!("String({want}) = {}", jsnum::to_string(x)));
        }
        let round = jsnum::round(x);
        let round_text = if round == 0.0 && round.is_sign_negative() { "-0".to_string() } else { jsnum::to_string(round) };
        if round_text != case["round"].as_str().unwrap() {
            failures.push(format!("Math.round({want}) = {round_text}, Node {}", case["round"]));
        }
        for (d, fixed) in case["fixed"].as_array().unwrap().iter().enumerate() {
            if jsnum::to_fixed(x, d) != fixed.as_str().unwrap() {
                failures.push(format!("({want}).toFixed({d}) = {}, Node {fixed}", jsnum::to_fixed(x, d)));
            }
        }
    }
    assert!(failures.is_empty(), "{} differences:\n{}", failures.len(), failures.join("\n"));
}

//! Fractional sort keys: compact, lexicographically-sortable strings that
//! slot between any two neighbours (`a0` < `a0V` < `a1`).
//!
//! **Vendored** from rocicorp/fractional-indexing (`src/index.js`, `main`
//! branch), CC0-1.0 (no rights reserved):
//! <https://github.com/rocicorp/fractional-indexing>. The port keeps
//! byte-level behaviour identical to upstream so our keys interoperate with
//! every other implementation of the same family (LexoRank-style base62 keys
//! with `A-Z`/`a-z` magnitude heads). The upstream defaults are kept exactly:
//! digits are `BASE62_DIGITS`, heads are `BASE52_DIGITS` (`A-Z` negative
//! lengths, `a-z` positive lengths), and the head counts towards the integer
//! part length (`c` → integer part of 4 chars, e.g. `c000`).
//!
//! Key format: `<head><int digits><float digits>`. Byte order under the
//! `ascii_bin` collation is exactly the alphabet order (`0-9` < `A-Z` < `a-z`
//! and every digit is single-byte ≤ 0x7A), so the `ascii_bin` collation on
//! the `sort_key` column is **load-bearing** — any case-insensitive collation
//! silently reorders keys.
//!
//! `sort_key` values are never derived from outside input: they come from
//! this module or the `0003` backfill (same format), so every key handed to
//! [`key_between`]/[`keys_between`] is already valid and the validation
//! errors are unreachable in production (they exist so a corrupt key fails
//! loudly instead of silently mis-sorting).
//!
//! [`needs_rebalance`] is the rebalance trigger: a freshly generated key
//! longer than [`REBALANCE_THRESHOLD`] means the parent's keys have bloated
//! between two neighbours; the caller rewrites the parent's keys evenly in
//! **two phases** (park every row at a unique temporary key that sits outside
//! the base62 key space, then assign final even keys) — a single-phase even
//! rewrite transiently violates `UNIQUE(parent, sort_key)` when row *i*'s new
//! key equals an un-rewritten row's old key.

use crate::error::{AppError, Internal};

/// Digit alphabet: ascending single-byte chars (`0-9` < `A-Z` < `a-z`), ASCII
/// byte order == value order — this is what makes `ascii_bin` load-bearing.
const BASE62_DIGITS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
/// Head alphabet (default integer-part heads): `A-Z` negative-length heads,
/// `a-z` positive-length heads, straddling at `Z`/`a` (length 2 each).
const BASE52_DIGITS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const DIGITS_LEN: usize = BASE62_DIGITS.len(); // 62
const HALF_HEADS: usize = BASE52_DIGITS.len() / 2; // 26
const ZERO: u8 = b'0';
const LAST: u8 = b'z';

/// Rewrite a parent's keys evenly once any generated key exceeds this length
/// (Jira rebalances at 128; we catch it at 64 — comfortably before
/// `VARCHAR(255)` would ever become the constraint).
pub const REBALANCE_THRESHOLD: usize = 64;

/// A key that is not a valid order key (empty, bad head, alphabet char,
/// smallest integer, or a float part ending in `0`). Maps to
/// [`AppError::Internal`] at the call site — it signals corrupt data, never a
/// client error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderKeyError(String);

impl OrderKeyError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for OrderKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid order key: {}", self.0)
    }
}

impl std::error::Error for OrderKeyError {}

impl From<OrderKeyError> for AppError {
    fn from(_: OrderKeyError) -> Self {
        AppError::Internal(Internal)
    }
}

/// Byte position of `byte` in `digits` (alphabets hold ascending, unique,
/// single-byte chars).
fn digit_index(digits: &str, byte: u8) -> Option<usize> {
    digits.bytes().position(|c| c == byte)
}

/// Total integer-part length (head included) for a head byte, per the
/// base52 head rules (`a-z` → `head - 'a' + 2`, `A-Z` → `'Z' - head + 2`).
fn get_integer_length(head: u8) -> Result<usize, OrderKeyError> {
    match digit_index(BASE52_DIGITS, head) {
        Some(i) => Ok(if i < HALF_HEADS {
            HALF_HEADS - i + 1
        } else {
            i - HALF_HEADS + 2
        }),
        None => Err(OrderKeyError::new(format!(
            "invalid order key head: {head}",
            head = head as char
        ))),
    }
}

/// The smallest integer key is the longest negative head followed by all
/// zeros (`A` + 26 `0`s); upstream rejects it in [`validate_order_key`].
fn is_smallest_integer(key: &str) -> bool {
    key.len() == 1 + HALF_HEADS
        && key.as_bytes()[0] == BASE52_DIGITS.as_bytes()[0]
        && key.as_bytes()[1..].iter().all(|&b| b == ZERO)
}

fn validate_integer(int: &str) -> Result<(), OrderKeyError> {
    let head = match int.as_bytes().first() {
        Some(&head) => head,
        None => return Err(OrderKeyError::new("empty integer part")),
    };
    if get_integer_length(head)? != int.len() {
        return Err(OrderKeyError::new(format!(
            "integer part length does not match its head: {int}"
        )));
    }
    Ok(())
}

/// Split off the integer part (head + integer digits); the float part is the
/// remainder.
fn get_integer_part(key: &str) -> Result<&str, OrderKeyError> {
    let head = match key.as_bytes().first() {
        Some(&head) => head,
        None => return Err(OrderKeyError::new("empty key")),
    };
    let int_len = get_integer_length(head)?;
    if int_len > key.len() {
        return Err(OrderKeyError::new(format!(
            "key shorter than its head's integer part: {key}"
        )));
    }
    Ok(&key[..int_len])
}

/// Validate a full order key (stricter than upstream by exactly one rule:
/// every byte must be in the base62 alphabet — upstream lets a non-alphabet
/// float char through and treats it as `0` downstream).
fn validate_order_key(key: &str) -> Result<(), OrderKeyError> {
    if key.is_empty() {
        return Err(OrderKeyError::new("empty key"));
    }
    if key.bytes().any(|b| digit_index(BASE62_DIGITS, b).is_none()) {
        return Err(OrderKeyError::new(format!(
            "key contains a char outside the base62 alphabet: {key}"
        )));
    }
    if is_smallest_integer(key) {
        return Err(OrderKeyError::new(format!(
            "smallest integer key is not allowed: {key}"
        )));
    }
    let int_part = get_integer_part(key)?;
    let float_part = &key[int_part.len()..];
    if float_part.ends_with(ZERO as char) {
        return Err(OrderKeyError::new(format!(
            "float part must not end in '0': {key}"
        )));
    }
    Ok(())
}

fn increment_integer(x: &str) -> Result<Option<String>, OrderKeyError> {
    validate_integer(x)?;
    let head = x.as_bytes()[0];
    let zero = ZERO as char;
    // Walk the integer digit run right-to-left, turning maxed digits into
    // zeros (`trailing`) until one can be bumped.
    let mut trailing = String::new();
    for i in (1..x.len()).rev() {
        let d = digit_index(BASE62_DIGITS, x.as_bytes()[i])
            .ok_or_else(|| OrderKeyError::new(format!("digit outside the alphabet: {x}")))?
            + 1;
        if d == DIGITS_LEN {
            trailing.insert(0, zero);
        } else {
            let mut out = String::with_capacity(x.len() + 1);
            out.push(head as char);
            out.push_str(&x[1..i]);
            out.push(BASE62_DIGITS.as_bytes()[d] as char);
            out.push_str(&trailing);
            return Ok(Some(out));
        }
    }
    // Carry out of the whole digit run; `trailing` is now all zeros. Move the
    // head one step towards the largest magnitude and grow/shrink the digit
    // run to match the new head's integer length.
    let head_index = digit_index(BASE52_DIGITS, head)
        .ok_or_else(|| OrderKeyError::new(format!("head outside the head alphabet: {x}")))?;
    if head_index + 1 >= BASE52_DIGITS.len() {
        // Already the largest integer.
        return Ok(None);
    }
    let h = BASE52_DIGITS.as_bytes()[head_index + 1];
    let length_delta = get_integer_length(h)? as isize - get_integer_length(head)? as isize;
    let mut out = String::with_capacity(trailing.len() + 2);
    out.push(h as char);
    if length_delta > 0 {
        out.push_str(&trailing);
        out.push(zero);
    } else if length_delta < 0 {
        out.push_str(trailing.get(1..).unwrap_or(""));
    } else {
        out.push_str(&trailing);
    }
    Ok(Some(out))
}

fn decrement_integer(x: &str) -> Result<Option<String>, OrderKeyError> {
    validate_integer(x)?;
    let head = x.as_bytes()[0];
    let last = LAST as char;
    // Walk the integer digit run right-to-left, turning underflowed digits
    // into the largest digit (`trailing`) until one can be dropped.
    let mut trailing = String::new();
    for i in (1..x.len()).rev() {
        let d = digit_index(BASE62_DIGITS, x.as_bytes()[i])
            .ok_or_else(|| OrderKeyError::new(format!("digit outside the alphabet: {x}")))?;
        if d == 0 {
            trailing.insert(0, last);
        } else {
            let mut out = String::with_capacity(x.len() + 1);
            out.push(head as char);
            out.push_str(&x[1..i]);
            out.push(BASE62_DIGITS.as_bytes()[d - 1] as char);
            out.push_str(&trailing);
            return Ok(Some(out));
        }
    }
    // Borrow out of the whole digit run; `trailing` is now all max digits.
    let head_index = digit_index(BASE52_DIGITS, head)
        .ok_or_else(|| OrderKeyError::new(format!("head outside the head alphabet: {x}")))?;
    if head_index == 0 {
        // Already the smallest integer.
        return Ok(None);
    }
    let h = BASE52_DIGITS.as_bytes()[head_index - 1];
    let length_delta = get_integer_length(h)? as isize - get_integer_length(head)? as isize;
    let mut out = String::with_capacity(trailing.len() + 2);
    out.push(h as char);
    if length_delta > 0 {
        out.push_str(&trailing);
        out.push(last);
    } else if length_delta < 0 {
        out.push_str(trailing.get(1..).unwrap_or(""));
    } else {
        out.push_str(&trailing);
    }
    Ok(Some(out))
}

/// Midpoint between two float parts (no common prefix left after the caller
/// consumed it). `a` may be empty; `b` is `Some` non-empty or `None`.
fn midpoint(a: &str, b: Option<&str>) -> Result<String, OrderKeyError> {
    let b = b.filter(|b| !b.is_empty());
    let zero = ZERO;
    if let Some(b) = b {
        if a >= b {
            return Err(OrderKeyError::new(format!(
                "lower bound >= upper bound: {a} >= {b}"
            )));
        }
        if b.as_bytes().last() == Some(&zero) {
            return Err(OrderKeyError::new(format!("upper bound ends in '0': {b}")));
        }
    }
    if a.as_bytes().last() == Some(&zero) {
        return Err(OrderKeyError::new(format!("lower bound ends in '0': {a}")));
    }
    if let Some(b) = b {
        // Remove the longest common prefix, padding `a` with zeros as we go.
        let mut n = 0;
        while let Some(&b_byte) = b.as_bytes().get(n) {
            if a.as_bytes().get(n).copied().unwrap_or(zero) == b_byte {
                n += 1;
            } else {
                break;
            }
        }
        if n > 0 {
            let mut out = String::with_capacity(n + 2);
            out.push_str(&b[..n]);
            out.push_str(&midpoint(a.get(n..).unwrap_or(""), Some(&b[n..]))?);
            return Ok(out);
        }
    }
    // The first digits (or lack of digit) differ.
    let digit_a = match a.as_bytes().first() {
        Some(&b) => digit_index(BASE62_DIGITS, b)
            .ok_or_else(|| OrderKeyError::new(format!("digit outside the alphabet: {a}")))?,
        None => 0,
    };
    let digit_b = match b {
        Some(b) => digit_index(BASE62_DIGITS, b.as_bytes()[0])
            .ok_or_else(|| OrderKeyError::new(format!("digit outside the alphabet: {b}")))?,
        None => DIGITS_LEN,
    };
    if digit_b - digit_a > 1 {
        // `Math.round(0.5 * sum)` with non-negative operands == `ceil(sum / 2)`.
        let mid = (digit_a + digit_b).div_ceil(2);
        return Ok((BASE62_DIGITS.as_bytes()[mid] as char).to_string());
    }
    if let Some(b) = b
        && b.len() > 1
    {
        return Ok(b[..1].to_string());
    }
    // `b` is null or a single digit: `a`'s first digit is the previous digit
    // to `b`, or `9` if `b` is null.
    let mut out = String::with_capacity(2);
    out.push(BASE62_DIGITS.as_bytes()[digit_a] as char);
    out.push_str(&midpoint(a.get(1..).unwrap_or(""), None)?);
    Ok(out)
}

/// Generate an order key that sorts strictly between `a` (lower bound) and
/// `b` (upper bound). `None` means the unbounded side (`key_between(None,
/// None) == "a0"`); reversed bounds are swapped automatically. The error is
/// unreachable for keys produced by this module or the `0003` backfill.
pub fn key_between(a: Option<&str>, b: Option<&str>) -> Result<String, OrderKeyError> {
    if let Some(a) = a {
        validate_order_key(a)?;
    }
    if let Some(b) = b {
        validate_order_key(b)?;
    }
    let (a, b) = match (a, b) {
        (Some(a), Some(b)) if a > b => (Some(b), Some(a)),
        (a, b) => (a, b),
    };
    // Equal bounds: no key can sort strictly between them. (Upstream
    // silently returns a key that is not between them at all; erroring is
    // strictly more correct.)
    if let (Some(a), Some(b)) = (a, b)
        && a == b
    {
        return Err(OrderKeyError::new(format!("bounds are equal: {a}")));
    }

    let Some(a) = a else {
        let Some(b) = b else {
            // The shortest positive head: the first character of the second
            // half of BASE52_DIGITS ('a').
            return Ok(format!("{}0", BASE52_DIGITS.as_bytes()[HALF_HEADS] as char));
        };
        let ib = get_integer_part(b)?;
        let fb = &b[ib.len()..];
        if is_smallest_integer(ib) {
            return Ok(format!("{ib}{}", midpoint("", Some(fb))?));
        }
        if ib < b {
            return Ok(ib.to_string());
        }
        return decrement_integer(ib)?
            .ok_or_else(|| OrderKeyError::new("cannot decrement any more"));
    };

    let Some(b) = b else {
        let ia = get_integer_part(a)?;
        let fa = &a[ia.len()..];
        if let Some(i) = increment_integer(ia)? {
            return Ok(i);
        }
        return Ok(format!("{ia}{}", midpoint(fa, None)?));
    };

    let ia = get_integer_part(a)?;
    let fa = &a[ia.len()..];
    let ib = get_integer_part(b)?;
    let fb = &b[ib.len()..];
    if ia == ib {
        return Ok(format!("{ia}{}", midpoint(fa, Some(fb))?));
    }
    if let Some(i) = increment_integer(ia)? {
        if i.as_str() < b {
            return Ok(i);
        }
        return Ok(format!("{ia}{}", midpoint(fa, None)?));
    }
    Err(OrderKeyError::new("cannot increment any more"))
}

/// Generate `n` distinct order keys in strictly ascending order between `a`
/// and `b`, evenly distributed (both `None` → consecutive integer keys).
pub fn keys_between(
    a: Option<&str>,
    b: Option<&str>,
    n: usize,
) -> Result<Vec<String>, OrderKeyError> {
    if n == 0 {
        return Ok(Vec::new());
    }
    if n == 1 {
        return Ok(vec![key_between(a, b)?]);
    }
    if b.is_none() {
        let mut result = Vec::with_capacity(n);
        let mut c = key_between(a, None)?;
        result.push(c.clone());
        for _ in 1..n {
            c = key_between(Some(&c), None)?;
            result.push(c.clone());
        }
        return Ok(result);
    }
    if a.is_none() {
        let mut result = Vec::with_capacity(n);
        let mut c = key_between(None, b)?;
        result.push(c.clone());
        for _ in 1..n {
            c = key_between(None, Some(&c))?;
            result.push(c.clone());
        }
        result.reverse();
        return Ok(result);
    }
    let mid = n / 2;
    let c = key_between(a, b)?;
    let mut out = keys_between(a, Some(&c), mid)?;
    out.push(c.clone());
    out.extend(keys_between(Some(&c), b, n - mid - 1)?);
    Ok(out)
}

/// Whether a freshly generated key signals key bloat in its parent (see the
/// module docs); the caller rewrites the parent's keys evenly in two phases.
pub fn needs_rebalance(key: &str) -> bool {
    key.len() > REBALANCE_THRESHOLD
}

/// Clamp a requested sibling index into `[0, upper]` (pure index clamp; the
/// wire contract clamps rather than 400s on out-of-range positions).
pub fn clamp_position(requested: i32, upper: i32) -> i32 {
    requested.clamp(0, upper.max(0))
}

/// Key strictly between the siblings at `index - 1` and `index` of an
/// ascending-sorted sibling key list (the list already EXCLUDES the moving
/// row — pass the list of the *other* siblings). `index == keys.len()`
/// appends at the end.
pub fn key_for_index(others_sorted: &[&str], index: usize) -> Result<String, OrderKeyError> {
    if index > others_sorted.len() {
        return Err(OrderKeyError::new(format!(
            "target index {index} is past the {} siblings",
            others_sorted.len()
        )));
    }
    let pred = index.checked_sub(1).map(|i| others_sorted[i]);
    let succ = others_sorted.get(index).copied();
    key_between(pred, succ)
}

#[cfg(test)]
mod tests {
    use super::{
        BASE62_DIGITS, REBALANCE_THRESHOLD, clamp_position, key_between, key_for_index,
        keys_between, needs_rebalance,
    };

    fn assert_strictly_increasing(keys: &[String]) {
        for pair in keys.windows(2) {
            assert!(pair[0] < pair[1], "keys strictly increasing: {pair:?}");
        }
    }

    fn between_both(a: &str, b: &str) -> String {
        key_between(Some(a), Some(b)).expect("valid neighbours")
    }

    #[test]
    fn empty_bounds_produce_the_classic_first_keys() {
        assert_eq!(key_between(None, None).expect("bounds"), "a0");
        assert_eq!(key_between(None, Some("a0")).expect("bounds"), "Zz");
        assert_eq!(key_between(Some("a0"), None).expect("bounds"), "a1");
    }

    #[test]
    fn midpoint_of_adjacent_integer_keys_is_the_mid_float() {
        assert_eq!(between_both("a0", "a1"), "a0V");
        assert_eq!(between_both("a1", "a2"), "a1V");
        assert_eq!(between_both("a0V", "a1"), "a0l");
        // Reversed bounds swap automatically (upstream behaviour).
        assert_eq!(between_both("a1", "a0"), "a0V");
        assert_eq!(between_both("Zz", "a0"), "ZzV");
    }

    #[test]
    fn matches_the_upstream_rocicorp_test_vectors() {
        // Pinned from a byte-compare of this port against
        // rocicorp/fractional-indexing (`src/index.js`, `main`) run 2026-11-15.
        for (a, b, expected) in [
            (None, None, "a0"),
            (None, Some("a0"), "Zz"),
            (Some("a0"), None, "a1"),
            (Some("a0"), Some("a1"), "a0V"),
            (Some("a1"), Some("a2"), "a1V"),
            (Some("a0V"), Some("a1"), "a0l"),
            (Some("a0"), Some("a0V"), "a0G"),
            (Some("a0V"), Some("a0W"), "a0VV"),
            (Some("Zz"), Some("a0"), "ZzV"),
            (Some("aZ"), Some("aZV"), "aZG"),
            (Some("aZ"), None, "aa"),
            (Some("az"), None, "b00"),
            (Some("c000"), Some("c001"), "c000V"),
            (Some("c00Z"), Some("c00a"), "c00ZV"),
            (Some("bZz"), Some("bZzV"), "bZzG"),
            (Some("a999"), Some("a99A"), "a999V"),
            (Some("Zzz"), None, "a0"),
            (Some("a0z"), None, "a1"),
        ] {
            assert_eq!(
                key_between(a, b).as_deref(),
                Ok(expected),
                "between {a:?} and {b:?}"
            );
        }
        assert_eq!(
            keys_between(None, Some("Zz"), 4).expect("bounds"),
            vec!["Zv", "Zw", "Zx", "Zy"]
        );
        assert_eq!(
            keys_between(Some("a0"), Some("a1"), 5).expect("bounds"),
            vec!["a08", "a0G", "a0V", "a0d", "a0l"]
        );
    }

    #[test]
    fn n_keys_between_null_null_are_consecutive_integers() {
        assert_eq!(
            keys_between(None, None, 5).expect("bounds"),
            vec!["a0", "a1", "a2", "a3", "a4"]
        );
        assert_eq!(
            keys_between(None, None, 0).expect("bounds"),
            Vec::<String>::new()
        );
        assert_eq!(keys_between(None, None, 1).expect("bounds"), vec!["a0"]);
    }

    #[test]
    fn n_keys_between_splits_evenly_and_stays_sorted() {
        for (a, b, n) in [
            (Some("a0"), Some("a1"), 3),
            (Some("a0"), Some("a1"), 20),
            (Some("Zz"), Some("a0"), 7),
            (Some("a0"), None, 4),
            (None, Some("a0"), 4),
            (Some("c000"), Some("c001"), 6),
            (Some("c000"), Some("c008"), 60),
        ] {
            let keys = keys_between(a, b, n).expect("bounds");
            assert_eq!(keys.len(), n, "exact count for n={n}");
            assert_strictly_increasing(&keys);
            if let Some(a) = a {
                assert!(
                    keys[0].as_str() > a,
                    "all keys above the lower bound: {keys:?}"
                );
            }
            if let Some(b) = b {
                assert!(
                    keys[n - 1].as_str() < b,
                    "all keys below the upper bound: {keys:?}"
                );
            }
        }
    }

    #[test]
    fn repeated_same_gap_growth_stays_strictly_between() {
        let mut upper = "a1".to_string();
        for _ in 0..16 {
            let key = key_between(Some("a0"), Some(&upper)).expect("between bounds");
            assert!(key.as_str() > "a0");
            assert!(key < upper);
            upper = key;
        }
    }

    #[test]
    fn increments_roll_over_digit_and_head_boundaries() {
        assert_eq!(key_between(Some("aZ"), None).expect("bounds"), "aa");
        assert_eq!(key_between(Some("az"), None).expect("bounds"), "b00");
        // `aZZ` parses as int part `aZ` + float `Z`: the float does not block
        // the integer increment.
        assert_eq!(key_between(Some("aZZ"), None).expect("bounds"), "aa");
        assert_eq!(key_between(Some("azZ"), None).expect("bounds"), "b00");
        // A long run of consecutive keys stays strictly increasing across the
        // `aZ…` → `aa…` and `az…` → `b00…` boundaries.
        let keys = keys_between(None, None, 200).expect("bounds");
        assert_strictly_increasing(&keys);
    }

    #[test]
    fn backfilled_keys_interoperate_with_key_between() {
        // Reproduce the `0003` backfill key shape in Rust (row n →
        // `c<d2><d1><d0>`, big-endian base62 over n) and prove that adjacent
        // pairs feed `key_between` into a strictly-between valid key.
        let digits = BASE62_DIGITS.as_bytes();
        let backfill_key = |n: u64| {
            let (d2, r) = ((n / 3844) as usize, (n % 3844) as usize);
            let (d1, d0) = (r / 62, r % 62);
            format!(
                "c{}{}{}",
                digits[d2] as char, digits[d1] as char, digits[d0] as char
            )
        };
        for n in [
            0_u64, 1, 61, 62, 63, 100, 3843, 3844, 3845, 10_000, 100_000, 238_326,
        ] {
            let lower = backfill_key(n);
            let upper = backfill_key(n + 1);
            assert!(lower < upper, "backfill keys monotonic: {lower} < {upper}");
            let between = key_between(Some(&lower), Some(&upper)).expect("between backfilled keys");
            assert!(lower < between, "between key {between} above {lower}");
            assert!(between < upper, "between key {between} below {upper}");
        }
    }

    #[test]
    fn invalid_keys_are_rejected() {
        for bad in [
            "",
            "a",                          // head declares a 2-digit int part, too short
            "!",                          // outside the alphabet
            "a00",                        // float part ends in '0'
            "a0!0",                       // non-alphabet float char
            "A0000000000000000000000000", // the smallest integer key
        ] {
            assert!(
                key_between(Some(bad), None).is_err(),
                "lower bound rejected: {bad:?}"
            );
        }
        assert!(
            key_between(Some("a0"), Some("a0")).is_err(),
            "equal bounds are an error"
        );
    }

    #[test]
    fn rebalance_threshold_triggers_only_past_64() {
        assert!(!needs_rebalance("a0"));
        assert!(!needs_rebalance(&"a".repeat(REBALANCE_THRESHOLD)));
        assert!(!needs_rebalance(&"a".repeat(REBALANCE_THRESHOLD - 1)));
        assert!(needs_rebalance(&"a".repeat(REBALANCE_THRESHOLD + 1)));
    }

    #[test]
    fn key_for_index_picks_the_right_gaps() {
        let keys = ["a0", "a1", "a2"];
        assert_eq!(key_for_index(&keys, 0).expect("gap"), "Zz");
        assert_eq!(key_for_index(&keys, 1).expect("gap"), "a0V");
        assert_eq!(key_for_index(&keys, 3).expect("gap"), "a3");
        // Out of range is an error, never a silent clamp.
        assert!(key_for_index(&keys, 4).is_err());
        // Empty sibling list: everything lands at the head.
        assert_eq!(key_for_index(&[], 0).expect("gap"), "a0");
        assert!(key_for_index(&[], 1).is_err());
    }

    #[test]
    fn clamp_position_pins_to_closed_range() {
        assert_eq!(clamp_position(2, 5), 2);
        assert_eq!(clamp_position(0, 5), 0);
        assert_eq!(clamp_position(5, 5), 5);
        assert_eq!(clamp_position(-1, 5), 0);
        assert_eq!(clamp_position(6, 5), 5);
        assert_eq!(clamp_position(999, 3), 3);
        assert_eq!(clamp_position(-50, 3), 0);
        assert_eq!(clamp_position(7, -4), 0);
    }

    #[test]
    fn bloated_neighbours_generate_an_over_threshold_key() {
        // Two adjacent rows deep in a repeated-zero run: the midpoint is
        // guaranteed to exceed the threshold, so the trigger can fire. Both
        // neighbours are valid keys (a float part may not end in '0').
        let lower = format!("a0{}1", "0".repeat(REBALANCE_THRESHOLD));
        let upper = format!("a0{}2", "0".repeat(REBALANCE_THRESHOLD));
        let between = key_between(Some(&lower), Some(&upper)).expect("between bloated bounds");
        assert!(
            needs_rebalance(&between),
            "generated key {between:?} exceeds {REBALANCE_THRESHOLD}"
        );
    }
}

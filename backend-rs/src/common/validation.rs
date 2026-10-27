//! Shared input validators (single owner).
//!
//! Registration (`POST /api/v1/auth/register`, T10) and `create-admin`
//! (T12) reuse these email + password fns so the two entry points cannot
//! drift (the Nest source drifted: `RegisterDto` used `MinLength` +
//! `Matches`, `create-admin` used `EMAIL_REGEX`/`PASSWORD_REGEX`). Feature
//! DTOs reference them from `#[garde(custom(...))]` attributes.
//!
//! Conventions (per `10-architecture.md` Validation + `20-api-contract.md`
//! catalogue):
//!
//! * Email uses a garde-shaped parser (approximates validator.js `isEmail`;
//!   borderline addresses such as single-label domains differ — accepted and
//!   recorded in `CONTRACT-CHANGES.md` by T52). It lives here rather than
//!   behind `#[garde(email)]` because garde's `email` cargo feature is off
//!   (enabling it would touch `Cargo.toml`/lockfile, outside T08 scope).
//! * Password **strength** (`>= 8` chars + digit/symbol) applies to
//!   register/create-admin inputs only.
//! * The password **byte cap** (`<= 72` bytes) applies to *every* password
//!   input including login: garde `length` counts chars while bcrypt
//!   truncates bytes, so a bare `MaxLength(72)` would silently collide
//!   multibyte passwords. Never use bare garde `length` on passwords.
//! * Login validates non-empty + byte cap only, so legacy weak passwords
//!   still reach 401 instead of 400.
//! * `not-blank` means empty after `trim()` (whitespace-only → 400), which is
//!   stricter than Nest's `IsNotEmpty` by design (see the catalogue).

/// Max wire length for emails (chars; mirrors Nest `@MaxLength(255)`).
pub const MAX_EMAIL_LEN: usize = 255;

/// Argon2/bcrypt byte ceiling for every password input.
///
/// bcrypt truncates past 72 bytes, so longer inputs must 400 rather than
/// silently collide (multibyte input can exceed 72 bytes well under 72
/// chars — hence bytes, never garde `length`, are measured here).
pub const MAX_PASSWORD_BYTES: usize = 72;

/// Minimum password characters for registration (chars, not bytes).
pub const MIN_PASSWORD_LEN: usize = 8;

/// Symbol set for the registration strength rule: the exact special-char
/// class from Nest `RegisterDto`'s `@Matches` lookahead (digit-or-symbol).
const PASSWORD_STRENGTH_SPECIALS: &str = "!@#$%^&*()_+-=[]{};':\"\\|,.<>/?";

/// Allowed `labels.color` values (exactly 6, mirrors `CreateLabelDto`).
pub const LABEL_COLORS: [&str; 6] = ["red", "orange", "yellow", "green", "blue", "purple"];

/// Allowed board `view_mode` values.
pub const VIEW_MODES: [&str; 2] = ["board", "list"];

/// Allowed column `sort_order` values.
pub const SORT_ORDERS: [&str; 2] = ["asc", "desc"];

/// Allowed note `type` values (`type` is derived from which FK is set; there
/// is no `type` column — see `20-api-contract.md` notes).
pub const NOTE_TYPES: [&str; 4] = ["general", "board", "project", "card"];

/// Normalize an email the way auth does before lookup/storage: trim +
/// lowercase (mirrors Nest `email.trim().to_lowerCase()`).
pub fn normalize_email(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// Shared email rule: garde-shaped parser (approximates validator.js
/// `isEmail`) plus the 255-char cap (chars, mirroring Nest `@MaxLength(255)`).
///
/// Shape mirrors `garde::rules::email` (whose `email` cargo feature is off
/// in this crate, so the check lives here instead of `#[garde(email)]`):
/// `user@domain` with an ASCII user part (`a-z0-9` plus
/// `. corroborate!#$%&'*+/=?^_`{|}~-`, `<= 64` bytes) and domain labels of
/// ASCII alphanumerics/hyphens (`<= 255` bytes, bracketed IP literals
/// accepted). Single-label domains (`a@b`) pass here while validator.js
/// `isEmail` requires a TLD — accepted divergence, recorded in
/// `CONTRACT-CHANGES.md` by T52.
pub fn validate_email(value: &str, _context: &()) -> garde::Result {
    if value.chars().count() > MAX_EMAIL_LEN {
        return Err(garde::Error::new("length: must be at most 255 characters"));
    }
    if let Err(reason) = check_email_shape(value) {
        return Err(garde::Error::new(format!("not a valid email: {reason}")));
    }
    Ok(())
}

fn check_email_shape(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("value is empty");
    }
    let (user, domain) = value.split_once('@').ok_or("value is missing `@`")?;
    if user.len() > 64 {
        return Err("user length exceeded maximum of 64 characters");
    }
    const USER_SPECIAL: &str = ".!#$%&'*+/=?^_`{|}~-";
    let user_ok = !user.is_empty()
        && user
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || USER_SPECIAL.as_bytes().contains(&byte));
    if !user_ok {
        return Err("user contains unexpected characters");
    }
    if domain.len() > 255 {
        return Err("domain length exceeded maximum of 255 characters");
    }
    if !check_domain(domain) {
        return Err("domain contains unexpected characters");
    }
    Ok(())
}

fn check_domain(domain: &str) -> bool {
    if domain.is_empty() {
        return false;
    }
    if let Some(inner) = domain
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        let literal = inner.strip_prefix("IPv6:").unwrap_or(inner);
        return literal.parse::<std::net::IpAddr>().is_ok();
    }
    domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

/// Registration/create-admin strength: `>= 8` chars plus at least one ASCII
/// digit or symbol from the Nest `@Matches` class. Never applied to login.
pub fn validate_register_password(value: &str, _context: &()) -> garde::Result {
    if value.chars().count() < MIN_PASSWORD_LEN {
        return Err(garde::Error::new("Password must be at least 8 characters"));
    }
    let has_digit_or_symbol = value
        .chars()
        .any(|c| c.is_ascii_digit() || PASSWORD_STRENGTH_SPECIALS.contains(c));
    if !has_digit_or_symbol {
        return Err(garde::Error::new(
            "Password must contain at least 1 number or special character",
        ));
    }
    Ok(())
}

/// Byte cap for *every* password input (register, login, create-admin):
/// at most 72 bytes (`str::len` counts bytes, never chars — that is the
/// point: multibyte input over 72 bytes 400s instead of silently truncating
/// under bcrypt).
pub fn validate_password_bytes(value: &str, _context: &()) -> garde::Result {
    if value.len() > MAX_PASSWORD_BYTES {
        return Err(garde::Error::new("Password must be at most 72 bytes"));
    }
    Ok(())
}

/// Login password rule: non-empty + byte cap only. Strength is deliberately
/// absent so legacy weak-but-wellformed passwords reach 401, never 400.
pub fn validate_login_password(value: &str, _context: &()) -> garde::Result {
    if value.is_empty() {
        return Err(garde::Error::new("must not be empty"));
    }
    validate_password_bytes(value, _context)
}

/// Non-blank after `trim()`: whitespace-only input 400s (stricter than
/// Nest `IsNotEmpty` by catalogue design).
pub fn validate_not_blank(value: &str, _context: &()) -> garde::Result {
    if value.trim().is_empty() {
        return Err(garde::Error::new("must not be blank"));
    }
    Ok(())
}

fn validate_optional(
    value: &Option<String>,
    rule: fn(&str, &()) -> garde::Result,
    context: &(),
) -> garde::Result {
    if let Some(inner) = value {
        rule(inner, context)?;
    }
    Ok(())
}

/// Optional variant of [`validate_not_blank`] for update DTOs (`None`
/// means "field untouched", `Some` must be non-blank).
pub fn validate_optional_not_blank(value: &Option<String>, context: &()) -> garde::Result {
    validate_optional(value, validate_not_blank, context)
}

/// Hex color in validator.js `isHexColor` shape: optional leading `#` plus
/// 3/4/6/8 hex digits (covers `#0079BF`, `#fff`).
pub fn validate_hex_color(value: &str, _context: &()) -> garde::Result {
    let digits = value.strip_prefix('#').unwrap_or(value);
    let ok = matches!(digits.len(), 3 | 4 | 6 | 8)
        && digits.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !ok {
        return Err(garde::Error::new("must be a valid hex color"));
    }
    Ok(())
}

/// Optional variant of [`validate_hex_color`] for update DTOs.
pub fn validate_optional_hex_color(value: &Option<String>, context: &()) -> garde::Result {
    validate_optional(value, validate_hex_color, context)
}

fn validate_one_of(value: &str, allowed: &[&str], what: &str) -> garde::Result {
    if !allowed.contains(&value) {
        return Err(garde::Error::new(format!(
            "{what} must be one of: {}",
            allowed.join(", ")
        )));
    }
    Ok(())
}

/// `labels.color` enum (exactly 6 values).
pub fn validate_label_color(value: &str, _context: &()) -> garde::Result {
    validate_one_of(value, &LABEL_COLORS, "color")
}

/// Optional variant of [`validate_label_color`] for update DTOs.
pub fn validate_optional_label_color(value: &Option<String>, context: &()) -> garde::Result {
    validate_optional(value, validate_label_color, context)
}

/// Board `view_mode` enum (`board` | `list`).
pub fn validate_view_mode(value: &str, _context: &()) -> garde::Result {
    validate_one_of(value, &VIEW_MODES, "view_mode")
}

/// Optional variant of [`validate_view_mode`] for update DTOs.
pub fn validate_optional_view_mode(value: &Option<String>, context: &()) -> garde::Result {
    validate_optional(value, validate_view_mode, context)
}

/// Column card `sort_order` enum (`asc` | `desc`).
pub fn validate_sort_order(value: &str, _context: &()) -> garde::Result {
    validate_one_of(value, &SORT_ORDERS, "sort_order")
}

/// Note `type` enum (`general` | `board` | `project` | `card`).
pub fn validate_note_type(value: &str, _context: &()) -> garde::Result {
    validate_one_of(value, &NOTE_TYPES, "type")
}

/// Optional variant of [`validate_note_type`] for the notes list query.
pub fn validate_optional_note_type(value: &Option<String>, context: &()) -> garde::Result {
    validate_optional(value, validate_note_type, context)
}

/// Card `due_date`: ISO-8601 valid-date-or-null. Accepts RFC 3339 datetimes
/// (e.g. `2026-01-01T00:00:00.000Z`, offsets) and calendar dates
/// (`YYYY-MM-DD`); explicit null arrives as `None`. `chrono` parsing rejects
/// impossible dates (`2026-02-30`) that JS `new Date()` also rejects — the
/// pinned-ISO-8601 delta is recorded in `CONTRACT-CHANGES.md` by T52.
pub fn validate_due_date(value: &Option<String>, _context: &()) -> garde::Result {
    let Some(raw) = value else {
        return Ok(());
    };
    if chrono::DateTime::parse_from_rfc3339(raw).is_ok() {
        return Ok(());
    }
    if chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").is_ok() {
        return Ok(());
    }
    if chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f").is_ok() {
        return Ok(());
    }
    Err(garde::Error::new(
        "due_date must be a valid ISO 8601 date string",
    ))
}

/// Note `tag_ids`: every id must be `>= 1` (mirrors Nest `@IsNumber(each)` +
/// the catalogue's int strictness; floats never reach here — serde rejects
/// them with 400 first).
pub fn validate_tag_ids(value: &Option<Vec<i32>>, _context: &()) -> garde::Result {
    if value
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|id| *id < 1)
    {
        return Err(garde::Error::new("tag id must be at least 1"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_accepts_wellformed_and_rejects_garbage() {
        for email in ["user@example.com", "a.b+tag@sub.example.co"] {
            assert!(validate_email(email, &()).is_ok(), "must accept: {email}");
        }
        for email in ["", "not-an-email", "a b@c.com", "user@.com"] {
            assert!(
                validate_email(email, &()).is_err(),
                "must reject: {email:?}"
            );
        }
        assert!(validate_email(&format!("{}@example.com", "x".repeat(250)), &()).is_err());
    }

    #[test]
    fn register_password_needs_length_and_digit_or_symbol() {
        for strong in ["P@ssw0rd1", "12345678", "no-digit-here!", "letters only 1"] {
            assert!(
                validate_register_password(strong, &()).is_ok(),
                "must accept: {strong:?}"
            );
        }
        for weak in ["short1!", "password", "1234567", "        ", "nodigits"] {
            assert!(
                validate_register_password(weak, &()).is_err(),
                "must reject: {weak:?}"
            );
        }
    }

    #[test]
    fn password_byte_cap_counts_bytes_not_chars() {
        assert!(validate_password_bytes(&"a".repeat(72), &()).is_ok());
        assert!(validate_password_bytes(&"a".repeat(73), &()).is_err());
        // 37 chars but 74 bytes: a bare char-length check would pass this.
        let multibyte = format!("a1{}", "é".repeat(36));
        assert_eq!(multibyte.chars().count(), 38);
        assert_eq!(multibyte.len(), 74);
        assert!(validate_password_bytes(&multibyte, &()).is_err());
        // Strength passes on its own here, isolating the byte cap.
        assert!(validate_register_password(&multibyte, &()).is_ok());
    }

    #[test]
    fn login_password_is_nonempty_plus_bytecap_only() {
        assert!(validate_login_password("weakpass", &()).is_ok());
        assert!(validate_login_password("", &()).is_err());
        assert!(validate_login_password(&"a".repeat(73), &()).is_err());
    }

    #[test]
    fn not_blank_rejects_whitespace_only() {
        assert!(validate_not_blank("x", &()).is_ok());
        for blank in ["", "   ", "\t\n"] {
            assert!(validate_not_blank(blank, &()).is_err());
        }
    }

    #[test]
    fn hex_color_shapes() {
        for color in ["#0079BF", "#fff", "0079BF", "#ffffffff"] {
            assert!(validate_hex_color(color, &()).is_ok(), "{color}");
        }
        for color in ["", "red", "#zzzzzz", "#12", "#12345"] {
            assert!(validate_hex_color(color, &()).is_err(), "{color:?}");
        }
    }

    #[test]
    fn enums_accept_only_members() {
        assert!(validate_label_color("red", &()).is_ok());
        assert!(validate_label_color("teal", &()).is_err());
        assert!(validate_view_mode("board", &()).is_ok());
        assert!(validate_view_mode("grid", &()).is_err());
        assert!(validate_sort_order("asc", &()).is_ok());
        assert!(validate_sort_order("ASC", &()).is_err());
        assert!(validate_note_type("card", &()).is_ok());
        assert!(validate_note_type("archive", &()).is_err());
    }

    #[test]
    fn due_date_accepts_iso_or_null_and_rejects_impossible_dates() {
        assert!(validate_due_date(&None, &()).is_ok());
        for date in [
            "2026-01-01T00:00:00.000Z",
            "2026-01-01",
            "2026-01-01T10:00:00+02:00",
        ] {
            assert!(
                validate_due_date(&Some(date.to_string()), &()).is_ok(),
                "{date}"
            );
        }
        for date in ["", "not-a-date", "2026-02-30", "2026-13-01"] {
            assert!(
                validate_due_date(&Some(date.to_string()), &()).is_err(),
                "{date:?}"
            );
        }
    }

    #[test]
    fn tag_ids_must_be_positive() {
        assert!(validate_tag_ids(&None, &()).is_ok());
        assert!(validate_tag_ids(&Some(vec![1, 2]), &()).is_ok());
        assert!(validate_tag_ids(&Some(vec![1, 0]), &()).is_err());
    }

    #[test]
    fn normalize_email_trims_and_lowercases() {
        assert_eq!(normalize_email("  User@Example.COM "), "user@example.com");
    }
}

//! auth DTOs: In = `Deserialize + garde::Validate + ToSchema`, Out = `Serialize + ToSchema`.
//!
//! Registration enforces email plus strength (`>= 8` chars with
//! digit/symbol) plus the 72-byte cap; login enforces email plus non-empty
//! plus byte cap only, so legacy weak passwords still reach 401 (T11)
//! instead of 400.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::ids::{Role, UserId};
use crate::common::validation::{
    validate_email, validate_login_password, validate_password_bytes, validate_register_password,
};

/// `POST /api/v1/auth/register` body (T10 handler).
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct RegisterInput {
    /// Email (garde parser ≈ validator.js `isEmail`, `<= 255` chars).
    #[garde(custom(validate_email))]
    pub email: String,
    /// New password: strength + byte cap (never bare garde `length`).
    #[garde(
        length(min = 8),
        custom(validate_register_password),
        custom(validate_password_bytes)
    )]
    pub password: String,
}

/// `POST /api/v1/auth/login` body (T11 handler): non-empty + byte cap only.
#[derive(Debug, Clone, Deserialize, garde::Validate, ToSchema)]
pub struct LoginInput {
    /// Email (same shared rule as registration).
    #[garde(custom(validate_email))]
    pub email: String,
    /// Existing password: no strength rule (weak-but-wellformed → 401).
    #[garde(custom(validate_login_password))]
    pub password: String,
}

/// Authenticated user shape shared by register/login/me responses:
/// `{data:{id,email,role}}` (no `message` envelope key — dropped per `20`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UserData {
    /// New `users.id` (`INT` ↔ `i32` newtype).
    pub id: UserId,
    /// Normalized email (`trim` + lowercase).
    pub email: String,
    // `value_type = String` keeps the OpenAPI shape `type: string` (no
    // `Role` component — the enum is deliberately not `ToSchema`). The doc
    // comment must stay verbatim: utoipa copies it into `openapi.json`.
    /// `user` on register (admin only via the T12 CLI).
    #[schema(value_type = String)]
    pub role: Role,
}

/// `POST /api/v1/auth/register → 201` body.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RegisterResponse {
    /// Created user.
    pub data: UserData,
}

/// `POST /api/v1/auth/login → 200` body (same shape as register, no
/// `message` envelope key — dropped per `20-api-contract.md`).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoginResponse {
    /// Authenticated user.
    pub data: UserData,
}

/// `GET /api/v1/auth/me → 200` body (fresh row values — the handler
/// re-looks-up the user; never the stale session payload).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MeResponse {
    /// Current user.
    pub data: UserData,
}

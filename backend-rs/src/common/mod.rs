//! Shared kernel helpers (T03: module stubs; implementations land in T04+).

pub mod card_summary;
pub mod ids;
pub mod label_seed;
pub mod ownership;
pub mod pagination;
/// The session store, owned in-tree (`54` Part C §C4).
pub mod session_store;
pub mod validation;

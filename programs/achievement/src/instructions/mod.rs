pub mod claim_achievement;
pub mod init_collection;
pub mod set_base_uri;

// Globs are required here: `#[derive(Accounts)]` also generates hidden
// `__client_accounts_*` / `__cpi_client_accounts_*` modules that `#[program]`
// resolves through this re-export. Naming only the Accounts structs breaks the
// macro. The `ambiguous_glob_reexports` warning on `handler` is the cost.
pub use claim_achievement::*;
pub use init_collection::*;
pub use set_base_uri::*;

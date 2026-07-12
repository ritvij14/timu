mod addresses;
mod authorized_keys;
mod cli;
mod error;
mod payload;
mod system;

pub use addresses::{
    AddressCandidate, AddressKind, choose_address, discover_addresses, select_address_candidates,
};
pub use authorized_keys::{
    CleanupGuard, append_authorized_key_line, build_temporary_authorized_key,
    reject_unsafe_authorized_keys_path, remove_tagged_authorization,
    replace_temporary_authorized_key, replace_temporary_authorized_key_in_file,
};
pub use cli::CliOptions;
pub use error::PayloadError;
pub use payload::{PairingPayload, is_expired, pairing_id_from_random_bytes};
pub use system::{CommandOutput, System, ensure_ssh_available, wait_for_completion};

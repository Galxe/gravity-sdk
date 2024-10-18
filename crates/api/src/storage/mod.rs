pub mod db;
pub mod db_options;
mod ledger_db;

use aptos_crypto::HashValue;
use aptos_types::proof::position::Position;
mod schema;
mod utils;
use anyhow::Result;

/// Defines the interface between `MerkleAccumulator` and underlying storage.
pub trait HashReader {
    /// Return `HashValue` carried by the node at `Position`.
    fn get(&self, position: Position) -> Result<HashValue>;
}

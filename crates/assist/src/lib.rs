//! Local terminal assistance: semantic ranking, command suggestions, and
//! paste-risk checks. The UI owns when these helpers run.

pub mod context;
pub mod model;
pub mod safety;
pub mod suggest;

pub use context::{lastblock, search, Block, Line};
pub use model::{compose, compose_match, explain};
pub use safety::{analyze, PasteRisk, RiskLevel};

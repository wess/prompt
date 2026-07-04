//! Local terminal assistance: semantic ranking, command suggestions, and
//! paste-risk checks. The UI owns when these helpers run.

pub mod context;
pub mod model;
pub mod safety;
pub mod suggest;
pub mod task;

pub use context::{blocktext, lastblock, search, Block, Line};
pub use model::{compose, explain};
pub use safety::{analyze, PasteRisk, RiskLevel};
pub use task::{Answer, Request};

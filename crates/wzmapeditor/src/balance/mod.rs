//! Per-player starting-economy balance: pure analysis, editor-side cache,
//! and the player color helper used by the panel and viewport overlay.

pub mod analysis;
pub mod state;

pub use analysis::{BalanceReport, PlayerBalance};
pub use state::{BalanceState, player_color};

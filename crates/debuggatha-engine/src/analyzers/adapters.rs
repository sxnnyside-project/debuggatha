//! The adapters: one per external tool, each saying how to run it, when it applies and how to read what it prints.

mod biome;
mod clippy;
mod eslint;
mod gitleaks;
mod kotlin;
mod osv;
mod phpstan;
mod ruff;
mod shared;

pub use biome::BIOME;
pub use clippy::CLIPPY;
pub use eslint::ESLINT;
pub use gitleaks::GITLEAKS;
pub use kotlin::{DETEKT, KTLINT};
pub use osv::OSV_SCANNER;
pub use phpstan::PHPSTAN;
pub use ruff::RUFF;

use super::types::Adapter;

pub static ADAPTERS: &[&Adapter] = &[
    &GITLEAKS,
    &BIOME,
    &ESLINT,
    &RUFF,
    &KTLINT,
    &DETEKT,
    &PHPSTAN,
    &CLIPPY,
    &OSV_SCANNER,
];

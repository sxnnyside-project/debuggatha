//! The detectors: each reads the semantic model or the source and turns a rule into hits.

pub mod calls;
pub mod concepts;
pub mod env;
pub mod env_docs;
pub mod functions;
pub mod js;
pub mod leaks;
pub mod modules;
pub mod promises;
pub mod secrets;
pub mod taint;
pub mod template;

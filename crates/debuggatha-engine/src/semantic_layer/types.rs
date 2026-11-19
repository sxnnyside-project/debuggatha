//! The semantic layer is optional and only ever adds opinions. A model may raise a finding no
//! analyzer can see, or say a finding looks like a false positive, but it never closes, dismisses
//! or hides anything: its verdict is evidence attached to a finding, and a person or a rule
//! decides what to do with it.

use serde::{Deserialize, Serialize};

/// What is asked of a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionRequest {
    /// The standing instructions.
    pub system: String,
    /// The code and the question.
    pub user: String,
    /// The most the model may answer with.
    pub max_tokens: u32,
}

/// Anything that can answer a prompt: a local Ollama, a local LM Studio, or the host's own model
/// through sampling.
pub trait SemanticProvider {
    /// Shown beside every opinion the model gave (`ollama`, `lmstudio`, `host`).
    fn name(&self) -> &str;
    /// The model that answered, when the provider knows it; a host reveals it with its first answer.
    fn model(&self) -> Option<String>;
    /// # Errors
    /// The model did not answer or answered with nothing; the message is safe to show.
    fn complete(&self, request: &CompletionRequest) -> Result<String, String>;
}

/// What the semantic pass may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticOptions {
    /// Attach the model's verdict to the candidates the built-in detectors are unsure of. Off by
    /// default: on the labeled set a 7B local model does not meet the bar, so a verdict is shown
    /// to a reader and never changes a finding.
    pub verify: bool,
    /// Read changed code for what analyzers do not see.
    pub detect: bool,
    /// At most this many findings are verified, most severe first.
    pub max_candidates: usize,
    /// At most this many files are read for detection.
    pub max_detect_files: usize,
    /// Lines of one file sent to the model.
    pub max_lines: usize,
}

impl Default for SemanticOptions {
    fn default() -> Self {
        Self {
            verify: false,
            detect: true,
            max_candidates: 10,
            max_detect_files: 3,
            max_lines: 250,
        }
    }
}

/// What the semantic pass did in one review, reported whether or not it found anything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticRun {
    /// The provider's name.
    pub provider: String,
    /// The model, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Findings a model was asked about.
    pub verified: u32,
    /// Of those, the ones it called real.
    pub confirmed: u32,
    /// The ones it called doubtful.
    pub doubtful: u32,
    /// The ones it could not tell.
    pub unsure: u32,
    /// Findings it raised that were kept.
    pub detected: u32,
    /// Claims the model made that did not match the code, and were dropped.
    pub rejected: u32,
    /// Calls that failed or answered with something unusable.
    pub failed: u32,
    /// What the person should know, such as a model that stopped answering.
    pub notes: Vec<String>,
    /// How long the pass took, in milliseconds.
    pub duration_ms: u64,
}

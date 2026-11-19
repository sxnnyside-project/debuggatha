//! The optional semantic pass: a model's opinion, attached to a review without ever closing,
//! hiding or lowering a finding. Off unless the person running the review turns it on.

mod judge;
mod providers;
mod review;
mod types;
mod view;

pub use judge::{
    DetectionResult, Verification, detection_prompt, detections_from, judge, parse_object,
    parse_verdict, verification_prompt, with_verdict,
};
pub use providers::{LocalProvider, LocalRuntime, assert_loopback, connect_local};
pub use review::{Refine, Scope, refine_findings};
pub use types::{CompletionRequest, SemanticOptions, SemanticProvider, SemanticRun};
pub use view::{CodeLine, CodeView, is_secret_file, render_view, scrub, window_around};

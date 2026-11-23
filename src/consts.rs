// =================================== NUMERICAL CONSTANTS ===================================

pub const MAX_REVIEW_COUNT: u8 = 1;

// =================================== AGENT PROMPT CONSTANTS ===================================

pub const REVIEW_INSTRUCTIONS: &str = include_str!("../prompts/REVIEWER.md");

pub const CODER_REVIEW_INSTRUCTIONS: &str = "Address the applicable review findings below. Keep changes scoped to the original request, then run the relevant tests.\n\n";

// =================================== NUMERICAL CONSTANTS ===================================

pub const MAX_REVIEW_COUNT: u8 = 1;

// =================================== AGENT PROMPT CONSANTS ===================================

pub const REVIEW_INSTRUCTIONS: &str = r#"Review the current Git diff against the original coding request below.

Look for bugs, regressions, and missing requirements introduced by the changes. Skip style preferences. For each finding, explain what goes wrong, when it happens, and cite the file and line if you can verify them. Do not edit files.

Return exactly one valid JSON object with two fields:
- "verdict": either "PASS" or "CHANGES_REQUESTED".
- "findings": a string. Use \n to separate multiple findings within the string.

Use "PASS" with an empty findings string when there are no actionable issues. Use "CHANGES_REQUESTED" with a nonempty findings string when there is at least one concrete issue.

Return raw JSON only, without Markdown fences or surrounding text.

Example passing response:
{"verdict":"PASS","findings":""}

Example response requesting changes:
{"verdict":"CHANGES_REQUESTED","findings":"src/main.rs:32: A second review can start while the first is still running."}

Original coding request:"#;

pub const CODER_REVIEW_INSTRUCTIONS: &str = "Address the applicable review findings below. Keep changes scoped to the original request, then run the relevant tests.\n\n";

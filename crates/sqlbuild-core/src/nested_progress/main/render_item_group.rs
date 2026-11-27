//! The bold model name that heads each run of test rows, with how many tests it has.

use crate::cli_text::models::CliStyle;

/// A blank line, then `  <group>  (N tests)`.
#[must_use]
pub fn render_item_group(
    group: &str,
    count: usize,
    (singular, plural): (&str, &str),
    style: CliStyle,
) -> String {
    let noun: &str = if count == 1 { singular } else { plural };
    format!(
        "\n  {}  {}\n",
        style.section(group),
        style.muted(&format!("({count} {noun})"))
    )
}

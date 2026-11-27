//! A line a running asset logged with `ctx.log`, printed as it arrives.

use crate::cli_text::models::CliStyle;

/// `  python    asset     <name> log  <message>`, warnings and errors naming their level; further lines of the message are indented under it.
#[must_use]
pub fn render_python_node_log_line(
    (kind, name): (&str, &str),
    (level, message): (&str, &str),
    style: CliStyle,
) -> String {
    let prefix: String = match level {
        "warning" | "error" => format!("{level}: "),
        _ => String::new(),
    };
    let mut lines = message.lines();
    let first: &str = lines.next().unwrap_or("");
    let mut text: String = format!(
        "  {:<10}{:<10}{name} {}{prefix}{first}\n",
        "python",
        kind,
        style.log_label("log  "),
    );
    for line in lines {
        text.push_str(&format!("{:<26}{line}\n", ""));
    }
    text
}

//! One refusal group as text: a header with its counts, notes and help once, then each origin.

use crate::cli_text::main::format_tree_connector::format_tree_connector;
use crate::cli_text::models::CliStyle;
use crate::format_command::main::render_help_lines::render_help_lines;
use crate::run_on::_helpers::refusal_groups::names_item;
use crate::run_on::constants::{
    EXPANDED_LABEL, NOTE_LABEL, RUN_ELSEWHERE_HELP_PREFIX, RUN_ELSEWHERE_HELP_SUFFIX, USED_BY_LABEL,
};
use crate::run_on::models::{LocalRefusal, RefusalGroup, RefusalOrigin};
use crate::text::main::text_wrap::text_wrap;

/// How an origin's models are listed: in full, or as a wrapped list that stops at two lines.
const COMPACT_LINES: usize = 2;
/// Stands in for a space inside a `code` span while help text is wrapped.
const CODE_SPACE: char = '\u{1f}';
/// A block's counts sit on its title line only with at least this many spaces before them.
const MIN_COUNT_GAP: usize = 2;
const MIN_WIDTH: usize = 30;

/// The text a group's block is rendered with.
pub(crate) struct BlockStyle<'a> {
    pub(crate) errors: bool,
    pub(crate) nouns: (&'a str, &'a str),
    pub(crate) verbose: bool,
    /// The width the head line's counts and the `used by` list are laid out to.
    pub(crate) columns: usize,
    /// The width notes wrap at: the terminal's, or never without one.
    pub(crate) wrap: usize,
    pub(crate) style: CliStyle,
}

fn plural(count: usize, (singular, plural): (&str, &str)) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

/// The block's lines, and whether a compact list left models out.
pub(crate) fn group_lines(group: &RefusalGroup, block: &BlockStyle<'_>) -> (Vec<String>, bool) {
    let style: CliStyle = block.style;
    let refusal: &LocalRefusal = &group.refusal;
    let severity: &str = if block.errors { "error" } else { "note" };
    let tag: String = match &refusal.code {
        Some(code) if !code.is_empty() => format!("{severity}[{code}]:"),
        _ => format!("{severity}:"),
    };
    let tag: String = if block.errors {
        style.error_strong(&tag)
    } else {
        style.accent_strong(&tag)
    };
    let message: String = if group.unplaced {
        refusal.category.clone()
    } else {
        refusal.message.clone().unwrap_or_else(|| refusal.title())
    };
    let mut counts: Vec<String> = vec![plural(group.items.len(), block.nouns)];
    let models: usize = group.models();
    if models > 0 {
        counts.push(plural(models, ("model", "models")));
    }
    if !group.unplaced && group.origins.len() > 1 {
        counts.push(plural(group.origins.len(), ("place", "places")));
    }
    let counts: String = counts.join(" \u{00b7} ");
    let head_width: usize = tag_width(refusal, severity) + 1 + message.chars().count();
    let mut lines: Vec<String> = Vec::new();
    let gap: usize = block
        .columns
        .saturating_sub(head_width + counts.chars().count());
    if gap >= MIN_COUNT_GAP {
        lines.push(format!(
            "{tag} {}{}{}",
            style.section(&message),
            " ".repeat(gap),
            style.muted(&counts)
        ));
    } else {
        lines.push(format!("{tag} {}", style.section(&message)));
        lines.push(format!("   {}", style.muted(&counts)));
    }
    if group.unplaced {
        lines.extend(construct_rows(group, block));
        if group.origins.len() == 1 {
            let mut refusal: LocalRefusal = group.refusal.clone();
            if refusal.notes.is_empty() && !refusal.reason.is_empty() {
                refusal.notes.push(refusal.reason.clone());
            }
            lines.extend(note_lines(&refusal, block));
        }
        lines.extend(group_help(&group.refusal, block));
        return (lines, false);
    }
    lines.extend(note_lines(refusal, block));
    lines.extend(group_help(refusal, block));
    let numbered: bool = group.origins.len() > 1;
    let mut hidden: bool = false;
    for (index, origin) in group.origins.iter().enumerate() {
        let indent: &str = if numbered { "      " } else { "   " };
        if numbered {
            lines.push(String::new());
            lines.push(origin_heading(index + 1, origin, block));
        }
        lines.extend(frame_lines(&origin.refusal, indent, style));
        if origin.refusal.macro_name.is_some() {
            let (users, cut) = used_by(origin, indent, block);
            lines.extend(users);
            hidden |= cut;
        }
    }
    (lines, hidden)
}

fn tag_width(refusal: &LocalRefusal, severity: &str) -> usize {
    match &refusal.code {
        Some(code) if !code.is_empty() => severity.len() + code.len() + 3,
        _ => severity.len() + 1,
    }
}

/// `1. @py.surrogate_key  macro, 25 tests · 10 models`, or `2. orders__stg__orders  2 tests`.
fn origin_heading(number: usize, origin: &RefusalOrigin, block: &BlockStyle<'_>) -> String {
    let style: CliStyle = block.style;
    let items: String = plural(origin.items.len(), block.nouns);
    match &origin.refusal.macro_name {
        Some(name) => format!(
            "   {} {}  {}",
            style.section(&format!("{number}.")),
            style.section(name),
            style.muted(&format!(
                "macro, {items} \u{00b7} {}",
                plural(origin.users.len(), ("model", "models"))
            ))
        ),
        None => {
            let model: String = origin
                .users
                .first()
                .map_or_else(String::new, |user| user.model.clone());
            format!(
                "   {} {}  {}",
                style.section(&format!("{number}.")),
                style.section(&model),
                style.muted(&items)
            )
        }
    }
}

/// The construct's place as a code frame: in the macro's own source when found, else where it was refused.
fn frame_lines(refusal: &LocalRefusal, indent: &str, style: CliStyle) -> Vec<String> {
    let label: &str = refusal.label.as_deref().unwrap_or(&refusal.reason);
    let (path, line, column, snippet, caret) = match &refusal.macro_place {
        Some(place) => (
            place.path.clone(),
            place.line,
            place.column,
            Some(place.snippet.clone()),
            place.column,
        ),
        None => match (&refusal.path, refusal.line, refusal.column) {
            (Some(path), Some(line), Some(column)) => (
                path.clone(),
                line,
                column,
                refusal.snippet.clone(),
                refusal.snippet_column.unwrap_or(column),
            ),
            _ => return Vec::new(),
        },
    };
    let number: String = line.to_string();
    let gutter: String = " ".repeat(number.chars().count());
    let mut lines: Vec<String> = vec![format!(
        "{indent}{gutter}{} {path}:{line}:{column}",
        style.link("-->")
    )];
    if let Some(snippet) = snippet {
        let carets: String = "^".repeat(usize::try_from(refusal.width.max(1)).unwrap_or(1));
        let caret_at: usize = usize::try_from((caret - 1).max(0)).unwrap_or(0);
        lines.push(format!("{indent}{gutter} {}", style.link("|")));
        lines.push(format!(
            "{indent}{} {} {snippet}",
            style.link(&number),
            style.link("|")
        ));
        lines.push(format!(
            "{indent}{gutter} {} {}{} {}",
            style.link("|"),
            " ".repeat(caret_at),
            style.error(&carets),
            style.error(label)
        ));
    }
    if refusal.macro_place.is_none()
        && let Some(expanded) = &refusal.expanded_from
    {
        lines.push(format!(
            "{indent}{gutter} {} {expanded}",
            style.muted(EXPANDED_LABEL)
        ));
    }
    lines
}

/// `= used by N models:` and the models, most items first.
fn used_by(origin: &RefusalOrigin, indent: &str, block: &BlockStyle<'_>) -> (Vec<String>, bool) {
    let style: CliStyle = block.style;
    let mut lines: Vec<String> = vec![format!(
        "{indent} {}",
        style.muted(&format!(
            "{USED_BY_LABEL} {}:",
            plural(origin.users.len(), ("model", "models"))
        ))
    )];
    let list_indent: String = format!("{indent}   ");
    if block.verbose {
        let width: usize = origin
            .users
            .iter()
            .map(|user| user.model.chars().count())
            .max()
            .unwrap_or(0);
        for (index, user) in origin.users.iter().enumerate() {
            lines.push(format!(
                "{list_indent}{} {:<width$}  {:<8} {}",
                format_tree_connector(style, index + 1 == origin.users.len()),
                user.model,
                plural(user.items.len(), block.nouns),
                style.muted(&format!(":{}", user.line))
            ));
        }
        return (lines, false);
    }
    let width: usize = block
        .columns
        .saturating_sub(list_indent.len())
        .max(MIN_WIDTH);
    let mut text: String = String::new();
    let mut shown: usize = 0;
    for user in &origin.users {
        let piece: String = format!("{} ({})", user.model, user.items.len());
        let next: String = if text.is_empty() {
            piece
        } else {
            format!("{text}, {piece}")
        };
        if text_wrap(&next, width, ("", ""), false).len() > COMPACT_LINES {
            break;
        }
        text = next;
        shown += 1;
    }
    let cut: bool = shown < origin.users.len();
    if cut {
        text = format!("{text}, +{} more", origin.users.len() - shown);
    }
    for line in text_wrap(&text, width, ("", ""), false) {
        lines.push(format!("{list_indent}{}", style.muted(&line)));
    }
    (lines, cut)
}

/// One row per construct of an unplaced group: its name, how many items, and the first of them.
fn construct_rows(group: &RefusalGroup, block: &BlockStyle<'_>) -> Vec<String> {
    let style: CliStyle = block.style;
    let name = |origin: &RefusalOrigin| {
        let first: &str = origin.items.first().map_or("", String::as_str);
        if names_item(&origin.refusal, first) {
            first.to_owned()
        } else {
            origin.refusal.construct.clone()
        }
    };
    let width: usize = group
        .origins
        .iter()
        .map(|origin| name(origin).chars().count())
        .max()
        .unwrap_or(0);
    group
        .origins
        .iter()
        .enumerate()
        .map(|(index, origin)| {
            let first: String = origin.items.first().cloned().unwrap_or_default();
            let others: String = match origin.items.len() {
                _ if names_item(&origin.refusal, &first) => String::new(),
                0 | 1 => first,
                count => format!("{first} +{}", count - 1),
            };
            let tail: String = if others.is_empty() {
                String::new()
            } else {
                format!(" {}", style.muted(&others))
            };
            format!(
                "   {} {:<width$}  {:<8}{tail}",
                format_tree_connector(style, index + 1 == group.origins.len()),
                name(origin),
                plural(origin.items.len(), block.nouns),
            )
            .trim_end()
            .to_owned()
        })
        .collect()
}

fn note_lines(refusal: &LocalRefusal, block: &BlockStyle<'_>) -> Vec<String> {
    let prefix: String = format!("   {NOTE_LABEL} ");
    let width: usize = block
        .wrap
        .saturating_sub(prefix.chars().count())
        .max(MIN_WIDTH);
    let mut lines: Vec<String> = Vec::new();
    for note in &refusal.notes {
        for (index, line) in text_wrap(note, width, ("", ""), false).iter().enumerate() {
            if index == 0 {
                lines.push(format!("   {} {line}", block.style.muted(NOTE_LABEL)));
            } else {
                lines.push(format!("{}{line}", " ".repeat(prefix.chars().count())));
            }
        }
    }
    lines
}

/// The cause's own help; under `--run-on local` a cause without one says to run its items on the warehouse.
fn group_help(refusal: &LocalRefusal, block: &BlockStyle<'_>) -> Vec<String> {
    match (&refusal.help, block.errors) {
        (None, true) => help_lines(
            &LocalRefusal {
                help: Some(format!(
                    "{RUN_ELSEWHERE_HELP_PREFIX}{}{RUN_ELSEWHERE_HELP_SUFFIX}",
                    block.nouns.1
                )),
                ..LocalRefusal::default()
            },
            block.style,
        ),
        _ => help_lines(refusal, block.style),
    }
}

/// The help under `= help:`, never wrapped inside a `code` span.
fn help_lines(refusal: &LocalRefusal, style: CliStyle) -> Vec<String> {
    refusal
        .help
        .as_deref()
        .map(|help| {
            render_help_lines(&keep_code_whole(help), style)
                .into_iter()
                .map(restore_code_spaces)
                .collect()
        })
        .unwrap_or_default()
}

/// A wrapped help line, indented one more column, with its code spans' spaces back.
fn restore_code_spaces(line: String) -> String {
    format!(" {}", line.replace(CODE_SPACE, " "))
}

/// `text` with each space inside a backtick span replaced by a character wrapping never breaks at.
fn keep_code_whole(text: &str) -> String {
    let mut inside: bool = false;
    text.chars()
        .map(|character| match character {
            '`' => {
                inside = !inside;
                character
            }
            ' ' if inside => CODE_SPACE,
            _ => character,
        })
        .collect()
}

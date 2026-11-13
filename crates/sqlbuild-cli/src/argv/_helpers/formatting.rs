//! `argparse.HelpFormatter`'s help, usage and version text in the Python 3.13 layout.

use sqlbuild_core::text::main::text_wrap::text_wrap;

use crate::argv::_helpers::text_wrap::normalize_whitespace;
use crate::argv::constants::{
    INDENT_INCREMENT, MAX_HELP_POSITION, MAXIMUM_BLANK_RUN, MINIMUM_TEXT_WIDTH, USAGE_PREFIX,
    VERSION_PLACEHOLDER,
};
use crate::argv::models::{ActionSpec, ArgvSettings, HelpText, Nargs, ParserSpec};

/// One formatter's widths: `HelpFormatter.__init__` and the longest invocation it has seen.
struct Formatter {
    width: i64,
    max_help_position: i64,
    action_max_length: i64,
}

impl Formatter {
    fn new(settings: &ArgvSettings) -> Self {
        let width: i64 = settings.columns - 2;
        Self {
            width,
            max_help_position: MAX_HELP_POSITION.min((width - 20).max(INDENT_INCREMENT * 2)),
            action_max_length: 0,
        }
    }
}

/// `parser.format_usage()`.
pub(crate) fn format_usage(settings: &ArgvSettings, parser: &ParserSpec) -> String {
    let formatter: Formatter = Formatter::new(settings);
    finish(&[usage(&formatter, parser)])
}

/// The `--version` action's output.
pub(crate) fn format_version(
    settings: &ArgvSettings,
    action: &ActionSpec,
    version: &str,
) -> String {
    let formatter: Formatter = Formatter::new(settings);
    let text: String = action
        .version
        .as_deref()
        .unwrap_or_default()
        .replace(VERSION_PLACEHOLDER, version);
    finish(&[format_text(&formatter, &text, 0)])
}

/// `parser.format_help()`.
pub(crate) fn format_help(settings: &ArgvSettings, parser: &ParserSpec) -> String {
    let mut formatter: Formatter = Formatter::new(settings);
    for action in parser
        .actions
        .iter()
        .filter(|action| action.help != HelpText::Suppress)
    {
        let entry: HelpEntry = help_entry(action);
        let mut length: i64 = entry.header.chars().count() as i64 + INDENT_INCREMENT;
        for subaction in &entry.subactions {
            length = length.max(subaction.header.chars().count() as i64 + 2 * INDENT_INCREMENT);
        }
        formatter.action_max_length = formatter.action_max_length.max(length);
    }
    let mut items: Vec<String> = vec![usage(&formatter, parser)];
    if let Some(description) = &parser.description {
        items.push(format_text(&formatter, description, 0));
    }
    for (index, group) in parser.groups.iter().enumerate() {
        let mut section: Vec<String> = Vec::new();
        if let Some(description) = &group.description {
            section.push(format_text(&formatter, description, INDENT_INCREMENT));
        }
        for action in parser
            .actions
            .iter()
            .filter(|action| action.group == index && action.help != HelpText::Suppress)
        {
            section.push(format_action(
                &formatter,
                &help_entry(action),
                INDENT_INCREMENT,
            ));
        }
        let body: String = section.concat();
        if body.is_empty() {
            continue;
        }
        let heading: String = group
            .title
            .as_ref()
            .map_or_else(String::new, |title| format!("{title}:\n"));
        items.push(format!("\n{heading}{body}\n"));
    }
    if let Some(epilog) = &parser.epilog {
        items.push(format_text(&formatter, epilog, 0));
    }
    finish(&items)
}

/// The root section's `format_help` followed by `HelpFormatter.format_help`'s clean-up.
fn finish(items: &[String]) -> String {
    let body: String = items.concat();
    if body.is_empty() {
        return String::new();
    }
    let mut text: String = String::with_capacity(body.len() + 2);
    let mut newlines: usize = 0;
    for character in format!("\n{body}\n").chars() {
        if character == '\n' {
            newlines += 1;
            if newlines > MAXIMUM_BLANK_RUN {
                continue;
            }
        } else {
            newlines = 0;
        }
        text.push(character);
    }
    format!("{}\n", text.trim_matches('\n'))
}

fn help_text(help: &HelpText) -> Option<&str> {
    match help {
        HelpText::Text(text) => Some(text),
        HelpText::None | HelpText::Suppress => None,
    }
}

/// `_format_text`: a filled paragraph at `indent`.
fn format_text(formatter: &Formatter, text: &str, indent: i64) -> String {
    let width: i64 = (formatter.width - indent).max(MINIMUM_TEXT_WIDTH);
    let padding: String = " ".repeat(indent as usize);
    let lines: Vec<String> = text_wrap(
        &normalize_whitespace(text),
        width as usize,
        (&padding, &padding),
        true,
    );
    format!("{}\n\n", lines.join("\n"))
}

/// One help line's invocation and text, with its indented sub-actions.
struct HelpEntry {
    header: String,
    help: Option<String>,
    subactions: Vec<HelpEntry>,
}

/// An action's help entry; `_iter_indented_subactions` gives a sub-command action's choices.
fn help_entry(action: &ActionSpec) -> HelpEntry {
    HelpEntry {
        header: invocation(action),
        help: help_text(&action.help).map(str::to_owned),
        subactions: action
            .choices_help
            .iter()
            .map(|choice| HelpEntry {
                header: choice.metavar.clone(),
                help: help_text(&choice.help).map(str::to_owned),
                subactions: Vec::new(),
            })
            .collect(),
    }
}

/// `_format_action`: one action's help entry and its sub-actions'.
fn format_action(formatter: &Formatter, entry: &HelpEntry, indent: i64) -> String {
    let help_position: i64 = (formatter.action_max_length + 2).min(formatter.max_help_position);
    let help_width: i64 = (formatter.width - help_position).max(MINIMUM_TEXT_WIDTH);
    let action_width: i64 = help_position - indent - 2;
    let padding: String = " ".repeat(indent as usize);
    let header: &str = &entry.header;
    let help: Option<&str> = entry.help.as_deref().filter(|text| !text.is_empty());
    let header_length: i64 = header.chars().count() as i64;
    let mut indent_first: i64 = 0;
    let mut text: String = match help {
        None => format!("{padding}{header}\n"),
        Some(_) if header_length <= action_width => {
            format!("{padding}{header:<width$}  ", width = action_width as usize)
        }
        Some(_) => {
            indent_first = help_position;
            format!("{padding}{header}\n")
        }
    };
    let lines: Vec<String> = help
        .map(|help| {
            text_wrap(
                &normalize_whitespace(help),
                help_width as usize,
                ("", ""),
                true,
            )
        })
        .unwrap_or_default();
    if let Some((first, rest)) = lines.split_first() {
        text.push_str(&format!("{}{first}\n", " ".repeat(indent_first as usize)));
        for line in rest {
            text.push_str(&format!("{}{line}\n", " ".repeat(help_position as usize)));
        }
    } else if !text.ends_with('\n') {
        text.push('\n');
    }
    for subaction in &entry.subactions {
        text.push_str(&format_action(
            formatter,
            subaction,
            indent + INDENT_INCREMENT,
        ));
    }
    text
}

/// `_metavar_formatter(action, default)(1)[0]`.
fn metavar(action: &ActionSpec, default: &str) -> String {
    match (&action.metavar, &action.choices) {
        (Some(Some(metavar)), _) => metavar.clone(),
        (Some(None), _) => crate::argv::constants::SUPPRESS_DEST.to_owned(),
        (None, Some(choices)) => format!("{{{}}}", choices.join(",")),
        (None, None) => default.to_owned(),
    }
}

/// `_format_args`.
fn format_args(action: &ActionSpec, default: &str) -> String {
    let name: String = metavar(action, default);
    match action.nargs {
        Nargs::Single => name,
        Nargs::Optional => format!("[{name}]"),
        Nargs::Attached => format!("[={name}]"),
        Nargs::ZeroOrMore => format!("[{name} ...]"),
        Nargs::OneOrMore => format!("{name} [{name} ...]"),
        Nargs::Parser => format!("{name} ..."),
        Nargs::Zero => String::new(),
    }
}

fn dest(action: &ActionSpec) -> &str {
    action
        .dest
        .as_deref()
        .unwrap_or(crate::argv::constants::SUPPRESS_DEST)
}

/// `_format_action_invocation`.
fn invocation(action: &ActionSpec) -> String {
    if action.option_strings.is_empty() {
        return metavar(action, dest(action));
    }
    let options: String = action.option_strings.join(", ");
    if action.nargs == Nargs::Zero {
        return options;
    }
    if action.nargs == Nargs::Attached {
        return format!(
            "{options}{}",
            format_args(action, &dest(action).to_uppercase())
        );
    }
    format!(
        "{options} {}",
        format_args(action, &dest(action).to_uppercase())
    )
}

/// `_format_usage`: the `usage:` line, wrapped to the terminal width.
fn usage(formatter: &Formatter, parser: &ParserSpec) -> String {
    let prog: &str = &parser.prog;
    if let Some(usage) = &parser.usage {
        return format!("{USAGE_PREFIX}{}\n\n", usage.replace("%(prog)s", prog));
    }
    if parser.actions.is_empty() {
        return format!("{USAGE_PREFIX}{prog}\n\n");
    }
    let mut ordered: Vec<&ActionSpec> = parser
        .actions
        .iter()
        .filter(|action| !action.option_strings.is_empty())
        .collect();
    let optional_count: usize = ordered.len();
    ordered.extend(
        parser
            .actions
            .iter()
            .filter(|action| action.option_strings.is_empty()),
    );
    let (parts, positional_start) = usage_parts(parser, &ordered, optional_count);
    let action_usage: String = parts.join(" ");
    let text: String = [prog, action_usage.as_str()]
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<&str>>()
        .join(" ");
    let prefix_length: usize = USAGE_PREFIX.len();
    let text_width: i64 = formatter.width;
    if (prefix_length + text.chars().count()) as i64 <= text_width {
        return format!("{USAGE_PREFIX}{text}\n\n");
    }
    let optional_parts: &[String] = &parts[..positional_start];
    let positional_parts: &[String] = &parts[positional_start..];
    let prog_length: usize = prog.chars().count();
    let lines: Vec<String> = if (prefix_length + prog_length) as f64 <= 0.75 * text_width as f64 {
        let indent: String = " ".repeat(prefix_length + prog_length + 1);
        let with_prog = |parts: &[String]| -> Vec<String> {
            let mut all: Vec<String> = vec![prog.to_owned()];
            all.extend(parts.iter().cloned());
            usage_lines(&all, &indent, Some(prefix_length), text_width)
        };
        if !optional_parts.is_empty() {
            let mut lines: Vec<String> = with_prog(optional_parts);
            lines.extend(usage_lines(positional_parts, &indent, None, text_width));
            lines
        } else if !positional_parts.is_empty() {
            with_prog(positional_parts)
        } else {
            vec![prog.to_owned()]
        }
    } else {
        let indent: String = " ".repeat(prefix_length);
        let mut lines: Vec<String> = usage_lines(&parts, &indent, None, text_width);
        if lines.len() > 1 {
            lines = usage_lines(optional_parts, &indent, None, text_width);
            lines.extend(usage_lines(positional_parts, &indent, None, text_width));
        }
        let mut all: Vec<String> = vec![prog.to_owned()];
        all.extend(lines);
        all
    };
    format!("{USAGE_PREFIX}{}\n\n", lines.join("\n"))
}

/// `get_lines` inside `_format_usage`.
fn usage_lines(
    parts: &[String],
    indent: &str,
    prefix_length: Option<usize>,
    text_width: i64,
) -> Vec<String> {
    let indent_length: i64 = indent.chars().count() as i64;
    let mut lines: Vec<String> = Vec::new();
    let mut line: Vec<&str> = Vec::new();
    let mut line_length: i64 = prefix_length.map_or(indent_length, |length| length as i64) - 1;
    for part in parts {
        let part_length: i64 = part.chars().count() as i64;
        if line_length + 1 + part_length > text_width && !line.is_empty() {
            lines.push(format!("{indent}{}", line.join(" ")));
            line.clear();
            line_length = indent_length - 1;
        }
        line.push(part);
        line_length += part_length + 1;
    }
    if !line.is_empty() {
        lines.push(format!("{indent}{}", line.join(" ")));
    }
    if prefix_length.is_some()
        && let Some(first) = lines.first_mut()
    {
        *first = first.chars().skip(indent_length as usize).collect();
    }
    lines
}

/// `_get_actions_usage_parts_with_split`: usage parts and where the positionals start.
fn usage_parts(
    parser: &ParserSpec,
    actions: &[&ActionSpec],
    optional_count: usize,
) -> (Vec<String>, usize) {
    let mut grouped: Vec<usize> = Vec::new();
    let mut inserts: Vec<(usize, usize, bool)> = Vec::new();
    for (group, required) in parser.mutex_groups.iter().enumerate() {
        let members: Vec<&ActionSpec> = parser
            .actions
            .iter()
            .filter(|action| action.mutex_group == Some(group))
            .collect();
        if members
            .iter()
            .all(|action| action.help == HelpText::Suppress)
        {
            continue;
        }
        let Some(start) = actions.iter().position(|action| action.id == members[0].id) else {
            continue;
        };
        let end: usize = start + members.len();
        let contiguous: bool = end <= actions.len()
            && actions[start..end]
                .iter()
                .zip(&members)
                .all(|(action, member)| action.id == member.id);
        if contiguous {
            grouped.extend(members.iter().map(|action| action.id));
            inserts.push((start, end, *required));
        }
    }
    let mut parts: Vec<Option<String>> = actions
        .iter()
        .map(|action| usage_part(action, grouped.contains(&action.id)))
        .collect();
    inserts.sort_by_key(|insert| std::cmp::Reverse((insert.0, insert.1)));
    let mut separated: Vec<usize> = Vec::new();
    for (start, end, required) in inserts {
        let mut group_parts: Vec<String> = parts[start..end].iter().flatten().cloned().collect();
        let size: usize = group_parts.len();
        let (open, close) = match (required, size > 1) {
            (true, true) => ("(", ")"),
            (true, false) => ("", ""),
            (false, _) => ("[", "]"),
        };
        group_parts[0] = format!("{open}{}", group_parts[0]);
        group_parts[size - 1] = format!("{}{close}", group_parts[size - 1]);
        for (offset, part) in group_parts[..size - 1].iter().enumerate() {
            let index: usize = start + offset;
            if !separated.contains(&index) {
                parts[index] = Some(format!("{part} |"));
                separated.push(index);
            }
        }
        parts[start + size - 1] = Some(group_parts[size - 1].clone());
        for part in &mut parts[start + size..end] {
            *part = None;
        }
    }
    let positional_start: usize = parts[..optional_count].iter().flatten().count();
    (parts.into_iter().flatten().collect(), positional_start)
}

/// One action's usage part, or `None` when its help is suppressed.
fn usage_part(action: &ActionSpec, grouped: bool) -> Option<String> {
    if action.help == HelpText::Suppress {
        return None;
    }
    if action.option_strings.is_empty() {
        let part: String = format_args(action, dest(action));
        if grouped && part.starts_with('[') && part.ends_with(']') {
            return Some(part[1..part.len() - 1].to_owned());
        }
        return Some(part);
    }
    let option: &str = &action.option_strings[0];
    let part: String = if action.nargs == Nargs::Zero {
        option.to_owned()
    } else if action.nargs == Nargs::Attached {
        format!(
            "{option}{}",
            format_args(action, &dest(action).to_uppercase())
        )
    } else {
        format!(
            "{option} {}",
            format_args(action, &dest(action).to_uppercase())
        )
    };
    if !action.required && !grouped {
        return Some(format!("[{part}]"));
    }
    Some(part)
}

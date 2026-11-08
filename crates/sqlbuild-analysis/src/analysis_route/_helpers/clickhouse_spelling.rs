//! A ClickHouse type as written, respelt so SQLBuild's type fallback gives Polyglot's canonical name.

use crate::analysis_route::constants::{
    CLICKHOUSE_TYPE_ALIASES, CLICKHOUSE_VERBATIM_TYPES, NULLABLE_WRAPPER,
};

/// `spelling` with each SQL alias Polyglot renames respelt, or as written when none applies.
pub(crate) fn clickhouse_spelling(spelling: &str) -> Option<String> {
    let written: &str = spelling.trim();
    if written.is_empty() {
        return None;
    }
    let mut renamed: bool = false;
    let respelt: Option<String> = respelt(written, &mut renamed);
    Some(match respelt {
        Some(respelt) if renamed => respelt,
        _ => written.to_owned(),
    })
}

/// One type: its name, renamed through the alias table, and its arguments, each type respelt.
fn respelt(written: &str, renamed: &mut bool) -> Option<String> {
    let (name, rest) = type_name(written)?;
    let rest: &str = rest.trim_start();
    if CLICKHOUSE_VERBATIM_TYPES
        .iter()
        .any(|verbatim| verbatim.eq_ignore_ascii_case(&name))
    {
        return Some(format!("{name}{rest}"));
    }
    let arguments: String = if rest.is_empty() {
        String::new()
    } else {
        let inner: &str = rest.strip_prefix('(')?.strip_suffix(')')?;
        let written: Vec<&str> = split_arguments(inner)?;
        let respelt: Vec<String> = written
            .iter()
            .map(|argument| argument_respelt(argument.trim(), renamed))
            .collect::<Option<_>>()?;
        if let ([only_written], [only]) = (written.as_slice(), respelt.as_slice())
            && name.eq_ignore_ascii_case(NULLABLE_WRAPPER)
            && !wraps_nullable(only_written.trim())
            && wraps_nullable(only)
        {
            return Some(only.clone());
        }
        format!("({})", respelt.join(", "))
    };
    match alias(&name) {
        Some(template) => {
            *renamed = true;
            Some(template.replacen("{}", &arguments, 1))
        }
        None => Some(format!("{name}{arguments}")),
    }
}

/// A literal argument as written, or a type argument respelt.
fn argument_respelt(argument: &str, renamed: &mut bool) -> Option<String> {
    let literal: bool = argument
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'\'' | b'-'));
    if literal {
        Some(argument.to_owned())
    } else {
        respelt(argument, renamed)
    }
}

/// The longest leading type name the alias table knows, or the first word, and the text after it.
fn type_name(written: &str) -> Option<(String, &str)> {
    let word_end = |text: &str| {
        text.find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(text.len())
    };
    let first_end: usize = word_end(written);
    if first_end == 0 {
        return None;
    }
    let after_first: &str = &written[first_end..];
    let second_start: usize = first_end + (after_first.len() - after_first.trim_start().len());
    let second_end: usize = second_start + word_end(&written[second_start..]);
    if second_end > second_start {
        let phrase: String = format!(
            "{} {}",
            &written[..first_end],
            &written[second_start..second_end]
        );
        if alias(&phrase).is_some() {
            return Some((phrase, &written[second_end..]));
        }
    }
    Some((written[..first_end].to_owned(), after_first))
}

/// The alias template for a case-insensitive type name: `{}` takes the arguments as written.
fn alias(name: &str) -> Option<&'static str> {
    CLICKHOUSE_TYPE_ALIASES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(name))
        .map(|(_, template)| *template)
}

/// The top-level comma-separated arguments, with commas inside quotes or parentheses kept.
fn split_arguments(inner: &str) -> Option<Vec<&str>> {
    let mut arguments: Vec<&str> = Vec::new();
    let (mut depth, mut quoted, mut start) = (0_usize, false, 0_usize);
    for (index, character) in inner.char_indices() {
        match character {
            '\'' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth = depth.checked_sub(1)?,
            ',' if !quoted && depth == 0 => {
                arguments.push(&inner[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    (depth == 0 && !quoted).then(|| {
        arguments.push(&inner[start..]);
        arguments
    })
}

/// Whether a type is a top-level `Nullable(...)`: Polyglot does not wrap an alias it makes nullable twice.
fn wraps_nullable(respelt: &str) -> bool {
    respelt
        .get(..=NULLABLE_WRAPPER.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(&format!("{NULLABLE_WRAPPER}(")))
}

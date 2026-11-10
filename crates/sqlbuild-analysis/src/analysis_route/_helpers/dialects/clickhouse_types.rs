//! ClickHouse type text: Polyglot-equal normalization, and declared and inferred types compared.

use crate::analysis_route::constants::{
    CLICKHOUSE_ALIASES, CLICKHOUSE_CASELESS_FAMILIES, CLICKHOUSE_DECIMAL_DEFAULT,
    CLICKHOUSE_DECIMAL_WIDTHS, CLICKHOUSE_EXACT_FAMILIES, CLICKHOUSE_NUMBER_FAMILIES,
    CLICKHOUSE_TYPE_ALIASES, CLICKHOUSE_VERBATIM_TYPES, CLICKHOUSE_WRAPPERS,
    LOW_CARDINALITY_WRAPPER, NULLABLE_WRAPPER,
};
use crate::analysis_route::models::TypeMatch;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeFamily};

/// `spelling` respelt through Polyglot's ClickHouse aliases, with Polyglot's family; `None` if empty.
pub(crate) fn clickhouse_spelling(spelling: &str) -> Option<NormalizedType> {
    let written: &str = spelling.trim();
    if written.is_empty() {
        return None;
    }
    let respelt: Option<String> = respelt(written)
        .filter(|(_, renamed)| *renamed)
        .map(|(text, _)| text);
    let text: &str = respelt.as_deref().unwrap_or(written);
    let mut normalized: NormalizedType = normalize_text_type(text, None);
    let family: Option<TypeFamily> = respelt
        .as_ref()
        .and(type_name(written))
        .and_then(|(name, _)| alias_family(&name));
    if let Some(family) = family {
        normalized.normalized_name = text
            .to_uppercase()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        normalized.family = family;
    }
    Some(normalized)
}

/// Polyglot's family for a top-level alias whose family the type fallback misreads.
fn alias_family(name: &str) -> Option<TypeFamily> {
    CLICKHOUSE_TYPE_ALIASES
        .iter()
        .find(|(alias, _, _)| alias.eq_ignore_ascii_case(name))
        .and_then(|(_, _, family)| *family)
}

/// One type respelt through the alias table, with whether any name in it was renamed.
fn respelt(written: &str) -> Option<(String, bool)> {
    let (name, rest) = type_name(written)?;
    let rest: &str = rest.trim_start();
    if CLICKHOUSE_VERBATIM_TYPES
        .iter()
        .any(|verbatim| verbatim.eq_ignore_ascii_case(&name))
    {
        return Some((format!("{name}{rest}"), false));
    }
    let (arguments, renamed): (String, bool) = if rest.is_empty() {
        (String::new(), false)
    } else {
        let inner: &str = rest.strip_prefix('(')?.strip_suffix(')')?;
        let written: Vec<&str> = split_arguments(inner)?;
        let respelt: Vec<(String, bool)> = written
            .iter()
            .map(|argument| argument_respelt(argument.trim()))
            .collect::<Option<_>>()?;
        let renamed: bool = respelt.iter().any(|(_, renamed)| *renamed);
        let respelt: Vec<String> = respelt.into_iter().map(|(text, _)| text).collect();
        if let ([only_written], [only]) = (written.as_slice(), respelt.as_slice())
            && name.eq_ignore_ascii_case(NULLABLE_WRAPPER)
            && !wraps_nullable(only_written.trim())
            && wraps_nullable(only)
        {
            return Some((only.clone(), renamed));
        }
        (format!("({})", respelt.join(", ")), renamed)
    };
    match alias(&name) {
        Some(template) => Some((template.replacen("{}", &arguments, 1), true)),
        None => Some((format!("{name}{arguments}"), renamed)),
    }
}

/// A literal argument as written, or a type argument respelt, with whether it was renamed.
fn argument_respelt(argument: &str) -> Option<(String, bool)> {
    let literal: bool = argument
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'\'' | b'-'));
    if literal {
        Some((argument.to_owned(), false))
    } else {
        respelt(argument)
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
        .find(|(alias, _, _)| alias.eq_ignore_ascii_case(name))
        .map(|(_, template, _)| *template)
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

/// A declared and an inferred ClickHouse type compared without top-level wrappers or spacing.
#[must_use]
pub(crate) fn clickhouse_type_match(
    declared: &str,
    inferred: Option<&str>,
    inferred_nullable: bool,
) -> TypeMatch {
    let (declared_base, declared_nullable) = unwrapped(declared);
    let Some(inferred) = inferred else {
        return TypeMatch::NotCompared {
            nullable_into_non_null: !declared_nullable && inferred_nullable,
        };
    };
    let (inferred_base, wrapped_nullable) = unwrapped(inferred);
    TypeMatch::Compared {
        same_type: unspaced(declared_base) == unspaced(inferred_base),
        nullable_into_non_null: !declared_nullable && (inferred_nullable || wrapped_nullable),
    }
}

/// `type_sql` without whitespace outside its quoted strings.
fn unspaced(type_sql: &str) -> String {
    let mut quoted: bool = false;
    type_sql
        .chars()
        .filter(|character| {
            if *character == '\'' {
                quoted = !quoted;
            }
            quoted || !character.is_whitespace()
        })
        .collect()
}

/// The type inside its top-level `Nullable` and `LowCardinality` wrappers, and whether it is nullable.
fn unwrapped(type_sql: &str) -> (&str, bool) {
    let mut current: &str = type_sql.trim();
    let mut nullable: bool = false;
    loop {
        if let Some(inner) = wrapped(current, NULLABLE_WRAPPER) {
            nullable = true;
            current = inner;
        } else if let Some(inner) = wrapped(current, LOW_CARDINALITY_WRAPPER) {
            current = inner;
        } else {
            return (current, nullable);
        }
    }
}

/// The argument of `wrapper(...)` when the whole of `type_sql` is that one call.
fn wrapped<'a>(type_sql: &'a str, wrapper: &str) -> Option<&'a str> {
    let head: &str = type_sql.get(..wrapper.len())?;
    if !head.eq_ignore_ascii_case(wrapper) {
        return None;
    }
    let inner: &str = type_sql[wrapper.len()..]
        .trim_start()
        .strip_prefix('(')?
        .strip_suffix(')')?;
    let mut depth: usize = 0;
    for character in inner.chars() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => return None,
            ')' => depth -= 1,
            _ => {}
        }
    }
    Some(inner.trim())
}

/// The type `spelling` denotes, as `toTypeName` spells it; `None` for a spelling it does not know.
pub(crate) fn clickhouse_type(spelling: &str) -> Option<String> {
    let text: &str = spelling.trim();
    let (base, arguments): (&str, Option<&str>) = match text.find('(') {
        Some(open) => (
            text[..open].trim(),
            Some(text[open + 1..].strip_suffix(')')?),
        ),
        None => (text, None),
    };
    let words: String = base.split_whitespace().collect::<Vec<&str>>().join(" ");
    if let Some(wrapper) = CLICKHOUSE_WRAPPERS
        .iter()
        .find(|wrapper| **wrapper == words)
    {
        return Some(format!("{wrapper}({})", clickhouse_type(arguments?)?));
    }
    let family: &str = family(&words)?;
    match (family, arguments) {
        ("String", _) => Some(family.to_owned()),
        ("Decimal", None) => Some(CLICKHOUSE_DECIMAL_DEFAULT.to_owned()),
        ("Decimal", Some(arguments)) => decimal(arguments),
        ("FixedString", None) => None,
        (_, None) => Some(family.to_owned()),
        (_, Some(_))
            if CLICKHOUSE_NUMBER_FAMILIES
                .iter()
                .any(|prefix| family.starts_with(prefix)) =>
        {
            Some(family.to_owned())
        }
        (_, Some(arguments)) => {
            let width: Option<&(&str, u32)> = CLICKHOUSE_DECIMAL_WIDTHS
                .iter()
                .find(|(name, _)| *name == family);
            match width {
                Some((_, precision)) => Some(format!("Decimal({precision}, {})", arguments.trim())),
                None => Some(format!("{family}({})", listed(arguments))),
            }
        }
    }
}

/// The family a type name or alias stands for, matched with case where ClickHouse does.
fn family(words: &str) -> Option<&'static str> {
    if let Some(exact) = CLICKHOUSE_EXACT_FAMILIES
        .iter()
        .find(|name| **name == words)
    {
        return Some(exact);
    }
    let upper: String = words.to_ascii_uppercase();
    CLICKHOUSE_CASELESS_FAMILIES
        .iter()
        .find(|name| name.to_ascii_uppercase() == upper)
        .copied()
        .or_else(|| {
            CLICKHOUSE_ALIASES
                .iter()
                .find(|(alias, _)| *alias == upper)
                .map(|(_, family)| *family)
        })
}

/// Type arguments as `toTypeName` writes them: trimmed, `, `-separated outside quoted strings.
fn listed(arguments: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current: String = String::new();
    let mut quoted: bool = false;
    for character in arguments.chars() {
        if character == ',' && !quoted {
            parts.push(current.trim().to_owned());
            current.clear();
            continue;
        }
        quoted ^= character == '\'';
        current.push(character);
    }
    parts.push(current.trim().to_owned());
    parts.join(", ")
}

/// `Decimal(P)` or `Decimal(P, S)`, as `Decimal(P, S)`.
fn decimal(arguments: &str) -> Option<String> {
    let parts: Vec<&str> = arguments.split(',').map(str::trim).collect();
    let numbers: Vec<u32> = match parts
        .iter()
        .map(|part| part.parse::<u32>())
        .collect::<Result<Vec<u32>, _>>()
    {
        Ok(numbers) => numbers,
        Err(_not_a_number) => return None,
    };
    match numbers.as_slice() {
        [precision] => Some(format!("Decimal({precision}, 0)")),
        [precision, scale] => Some(format!("Decimal({precision}, {scale})")),
        _ => None,
    }
}

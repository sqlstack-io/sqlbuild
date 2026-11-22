//! Hostile expansions spliced at every position, and what their code must still read as.

use crate::sql_scan::main::bounded_expansions::bounded_expansions;
use crate::sql_scan::main::dialect_non_code_ranges::dialect_non_code_ranges;
use crate::sql_scan::models::LexicalSyntax;

/// Negatives, comments, comment halves, operators, quotes, empty text and newlines.
const HOSTILE: [&str; 44] = [
    "",
    " ",
    "\n",
    "1",
    "-1",
    "- 1",
    "-",
    "--",
    "---",
    "-/",
    "/",
    "//",
    "/ 2",
    "*",
    "**",
    "*/",
    "/**/",
    "/* note */",
    "/* two\nlines */",
    "1 -- one",
    "1 -- one\n",
    "-- one",
    "-- one\n2",
    "1 // one",
    "//x",
    "#",
    "#x",
    "# x",
    "1 # one",
    "#!x",
    "!x",
    "+",
    "<",
    ">",
    "|",
    "'a--b'",
    "'/*'",
    "1-",
    "1/",
    "\n-1",
    "-1\n",
    "x -- a -- b",
    "1 /* c */ -- d",
    "\u{e9}-",
];
/// `{}` marks each site, all of them in code.
const POSITIONS: [&str; 25] = [
    "{}",
    "select {} as x, 7 as y",
    "select -{} as x, 7 as y",
    "select {}- 1",
    "select {}-1",
    "select a/{}",
    "select {}/2",
    "select {}*2",
    "select a/{}*2",
    "select {} -- note",
    "select {} /* note */",
    "select {} /* two\nlines */",
    "select {} /* note */ , b",
    "select {}\n, b",
    "select {}\r\n, b",
    "select {}  \n, b",
    "({})",
    "f({}, 2)",
    "{}_count",
    "select {}'text'",
    "select '--' || {}",
    "/* c */{}",
    "select {}{}",
    "select -{}-1, {}",
    "select {} {}\nfrom t",
];

/// `template` with `values` at its sites, as authored and once bounded.
pub(crate) fn spliced(template: &str, values: &[&str], syntax: &LexicalSyntax) -> (String, String) {
    let mut sql: String = String::new();
    let mut sites: Vec<(usize, usize, &str)> = Vec::new();
    let mut pieces = template.split("{}");
    sql.push_str(pieces.next().unwrap_or_default());
    for (piece, value) in pieces.zip(values) {
        sites.push((sql.len(), sql.len() + 2, value));
        sql.push_str("@v");
        sql.push_str(piece);
    }
    let mut texts: Vec<String> = values.iter().map(|value| (*value).to_owned()).collect();
    for expansion in bounded_expansions(&sql, &sites, Some(syntax)) {
        assert_eq!(
            &expansion.text[expansion.lead..expansion.lead + values[expansion.index].len()],
            values[expansion.index]
        );
        texts[expansion.index] = expansion.text;
    }
    let joined = |texts: &[String]| {
        template
            .split("{}")
            .zip(texts.iter().map(String::as_str).chain([""]))
            .flat_map(|(piece, value)| [piece, value])
            .collect::<String>()
    };
    let raw: Vec<String> = values.iter().map(|value| (*value).to_owned()).collect();
    (joined(&raw), joined(&texts))
}

/// The code and quoted text of `sql`, without whitespace or comments.
fn code(sql: &str, syntax: &LexicalSyntax) -> String {
    let mut kept: String = String::new();
    let mut at: usize = 0;
    for (start, end) in dialect_non_code_ranges(sql, syntax) {
        kept.extend(
            sql[at..start]
                .chars()
                .filter(|character| !character.is_whitespace()),
        );
        let construct: &str = &sql[start..end];
        kept.extend(
            Some(construct)
                .filter(|text| text.starts_with(['\'', '"', '`']))
                .into_iter(),
        );
        at = end;
    }
    kept.extend(
        sql[at..]
            .chars()
            .filter(|character| !character.is_whitespace()),
    );
    kept
}

/// Each splice whose bounded text reads wrong, or changed although it read right.
pub(crate) fn generated_failures(syntax: &LexicalSyntax) -> Vec<String> {
    POSITIONS
        .iter()
        .flat_map(|template| {
            HOSTILE.iter().flat_map(move |first| {
                HOSTILE
                    .iter()
                    .map(move |second| (*template, [*first, *second, *first]))
            })
        })
        .filter_map(|(template, values)| {
            let sites: usize = template.matches("{}").count();
            let values: &[&str] = &values[..sites];
            let expected: String = template
                .split("{}")
                .zip(values.iter().copied().chain([""]))
                .map(|(piece, value)| code(piece, syntax) + &code(value, syntax))
                .collect();
            let (raw, bounded) = spliced(template, values, syntax);
            let read_right: bool = code(&raw, syntax) == expected;
            let wrong: bool = code(&bounded, syntax) != expected || (read_right && bounded != raw);
            wrong.then(|| format!("{template:?} with {values:?} gave {bounded:?}"))
        })
        .collect()
}

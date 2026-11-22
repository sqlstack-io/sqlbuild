//! Shapes holding an obviously fake value, and NUL placed through them.

pub(crate) const NAMED_SHAPES: [&str; 8] = [
    "password=zebraquokkawombat",
    "pwd=zebraquokkawombat",
    "api_key: zebraquokkawombat",
    "Bearer zebraquokkawombat",
    "Authorization: Basic zebraquokkawombat",
    "PASSWORD 'zebraquokkawombat'",
    "IDENTIFIED BY 'zebraquokkawombat'",
    "postgres://orders:zebraquokkawombat@warehouse.local.test/orders",
];
pub(crate) const ERROR_SHAPES: [&str; 9] = [
    "password=zebraquokkawombat",
    "pwd=zebraquokkawombat",
    "api_key: zebraquokkawombat",
    "Bearer zebraquokkawombat",
    "Authorization: Basic zebraquokkawombat",
    "PASSWORD 'zebraquokkawombat'",
    "IDENTIFIED BY 'zebraquokkawombat'",
    "postgres://orders:zebraquokkawombat@warehouse.local.test/orders",
    "postgres://orders:zebraquokkawombat",
];
pub(crate) const SHAPE_LEADS: [&str; 5] = ["", "x", "17", "caf\u{e9}", "ValueError: refused "];

/// Whether any part of the fake value reads in `text`, with or without NUL.
pub(crate) fn shows_value(text: &str) -> bool {
    let read: String = text.replace('\0', "");
    ["zebra", "quokka", "wombat"]
        .iter()
        .any(|part| read.contains(part))
}

/// `text` with one NUL at each character boundary.
pub(crate) fn nul_positions(text: &str) -> impl Iterator<Item = String> + '_ {
    text.char_indices()
        .map(|(index, _)| index)
        .chain([text.len()])
        .map(|index| format!("{}\0{}", &text[..index], &text[index..]))
}

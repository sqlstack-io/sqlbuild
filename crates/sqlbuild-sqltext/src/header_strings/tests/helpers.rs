/// The characters of `text`.
pub(crate) fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

/// Every text of up to `rounds` of `pieces` in a row.
pub(crate) fn piece_texts(pieces: &[&str], rounds: usize) -> Vec<String> {
    let mut texts: Vec<String> = vec![String::new()];
    for _ in 0..rounds {
        texts = texts
            .iter()
            .flat_map(|text| pieces.iter().map(move |piece| format!("{text}{piece}")))
            .chain(texts.iter().cloned())
            .collect();
    }
    texts
}

/// Where the key before `index` ends, past any header whitespace.
fn key_end_before(text: &[char], index: usize) -> usize {
    text[..index]
        .iter()
        .rposition(|unit| !" \t\r\n".contains(*unit))
        .map_or(0, |last| last + 1)
}

/// Whether `text` holds `key`, in any letter case, as a whole word ending at `end`.
fn whole_key_at(text: &[char], end: usize, key: &str) -> bool {
    end >= key.len()
        && text[end - key.len()..end]
            .iter()
            .collect::<String>()
            .eq_ignore_ascii_case(key)
        && (end == key.len() || {
            let before: char = text[end - key.len() - 1];
            !(before.is_alphanumeric() || before == '_')
        })
}

/// The backward scan: under `meta` when a parenthesis still open before `end` follows `meta`.
fn inside_meta_backwards(text: &[char], end: usize) -> bool {
    (0..end)
        .rev()
        .try_fold(0_usize, |depth, index| {
            let opening: bool = text[index] == '(';
            let found: bool =
                opening && depth == 0 && whole_key_at(text, key_end_before(text, index), "meta");
            (!found)
                .then(|| {
                    depth + usize::from(text[index] == ')') - usize::from(opening && depth > 0)
                })
                .ok_or(())
        })
        .is_err()
}

/// Whether the string at `start` follows a `description` key outside `meta`, scanned backwards.
pub(crate) fn follows_backwards(text: &[char], start: usize) -> bool {
    let key: &str = "description";
    let key_end: usize = key_end_before(text, start);
    key_end < start
        && whole_key_at(text, key_end, key)
        && !inside_meta_backwards(text, key_end - key.len())
}

/// `starts` in the order given.
pub(crate) fn in_order(starts: Vec<usize>) -> Vec<usize> {
    starts
}

/// `starts` last first.
pub(crate) fn reversed(starts: Vec<usize>) -> Vec<usize> {
    starts.into_iter().rev().collect()
}

//! A `check_every` interval such as `15m` or `1h30m`, in seconds.

use crate::discovered_sets::constants::{CHECK_EVERY_UNITS, MIN_CHECK_EVERY_SECONDS};

/// `text` as whole seconds: amounts in `d`, `h`, `m` and `s`, largest unit first, at least a minute.
pub fn parse_check_every(text: &str) -> Result<u64, String> {
    let refused = || {
        format!(
            "check_every '{text}' is not an interval such as \"15m\" or \"1h30m\" of at least {MIN_CHECK_EVERY_SECONDS} seconds"
        )
    };
    let mut seconds: u64 = 0;
    let mut digits: String = String::new();
    let mut next_unit: usize = 0;
    for character in text.trim().chars() {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        let position = CHECK_EVERY_UNITS[next_unit..]
            .iter()
            .position(|(unit, _)| *unit == character)
            .ok_or_else(refused)?;
        let amount: u64 = digits.parse().map_err(|_| refused())?;
        let (_, unit_seconds) = CHECK_EVERY_UNITS[next_unit + position];
        seconds = amount
            .checked_mul(unit_seconds)
            .and_then(|added| seconds.checked_add(added))
            .ok_or_else(refused)?;
        digits.clear();
        next_unit += position + 1;
    }
    if !digits.is_empty() || seconds < MIN_CHECK_EVERY_SECONDS {
        return Err(refused());
    }
    Ok(seconds)
}

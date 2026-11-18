//! Python's `f"{value:,}"`: an integer with comma thousands separators.

/// The integer with a comma between each group of three digits, e.g. `-31,204`.
#[must_use]
pub fn format_thousands(value: i128) -> String {
    let digits: String = value.unsigned_abs().to_string();
    let mut grouped: String = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if value < 0 {
        grouped.push('-');
    }
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

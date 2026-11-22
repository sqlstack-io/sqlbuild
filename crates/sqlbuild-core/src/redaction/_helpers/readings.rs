//! Text with NUL reads two ways: as it is, and without NUL as it is stored.

use crate::redaction::constants::WITHHELD_TEXT;

/// `redact`'s text; with NUL, the whole text is withheld when either reading would change, and otherwise loses its NUL.
pub(crate) fn redacted_readings(text: &str, redact: impl Fn(&str) -> String) -> String {
    if !text.contains('\0') {
        return redact(text);
    }
    let without_nul: String = text.replace('\0', "");
    if redact(text) != text || redact(&without_nul) != without_nul {
        return WITHHELD_TEXT.to_owned();
    }
    without_nul
}

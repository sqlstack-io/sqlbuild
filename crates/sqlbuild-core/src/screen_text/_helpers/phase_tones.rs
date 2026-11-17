//! The glyph and colours of each phase state.

use crate::screen_text::constants::{
    FAIL_GLYPH, OK_GLYPH, PENDING_GLYPH, SPINNER_FRAMES, WARN_GLYPH,
};
use crate::screen_text::models::{PhaseState, Tone};

/// How one phase line paints its glyph, label, detail and time.
pub(crate) struct PhaseTones {
    pub(crate) glyph: &'static str,
    pub(crate) glyph_tone: Tone,
    pub(crate) label: Tone,
    pub(crate) detail: Tone,
    pub(crate) time: Tone,
}

/// Dim while pending, a yellow spinner and timer while running, then green, yellow or red.
pub(crate) fn phase_tones(state: PhaseState) -> PhaseTones {
    let (glyph, glyph_tone): (&'static str, Tone) = match state {
        PhaseState::Pending => (PENDING_GLYPH, Tone::Muted),
        PhaseState::Running { frame } => {
            (SPINNER_FRAMES[frame % SPINNER_FRAMES.len()], Tone::Warning)
        }
        PhaseState::Ok => (OK_GLYPH, Tone::Success),
        PhaseState::Warn => (WARN_GLYPH, Tone::Warning),
        PhaseState::Fail => (FAIL_GLYPH, Tone::Error),
    };
    PhaseTones {
        glyph,
        glyph_tone,
        label: if state == PhaseState::Pending {
            Tone::Muted
        } else {
            Tone::Plain
        },
        detail: match state {
            PhaseState::Warn => Tone::Warning,
            PhaseState::Fail => Tone::Error,
            _ => Tone::Muted,
        },
        time: if matches!(state, PhaseState::Running { .. }) {
            Tone::Warning
        } else {
            Tone::Muted
        },
    }
}

//! The card the `sqb dev` checklist collapses into once the location is ready.

use crate::screen_text::_helpers::rows::{column, elapsed_text, linked, render_row, span};
use crate::screen_text::constants::{
    CARD_GLYPH, CARD_LABEL_WIDTH, CONNECTED_GLYPH, DISCONNECTED_GLYPH, PRODUCT, SEPARATOR,
};
use crate::screen_text::models::{ReadyCard, ScreenFacts, Segment, Tone, WarehouseState};

/// The headline, a blank line, then one `➜  Label  value` row per fact.
#[must_use]
pub fn render_ready_card(card: &ReadyCard, facts: &ScreenFacts) -> Vec<String> {
    let warehouse: Vec<Segment> = match &card.warehouse {
        WarehouseState::Connected { detail } => vec![
            span(CONNECTED_GLYPH, Tone::Success),
            span(format!(" {}{SEPARATOR}{detail}", card.adapter), Tone::Plain),
        ],
        WarehouseState::NotConnected { reason } => vec![
            span(DISCONNECTED_GLYPH, Tone::Warning),
            span(
                format!(" not connected{SEPARATOR}{reason}{SEPARATOR}press s to retry"),
                Tone::Plain,
            ),
        ],
        WarehouseState::Failed { message } => vec![
            span(CONNECTED_GLYPH, Tone::Error),
            span(format!(" {}{SEPARATOR}", card.adapter), Tone::Plain),
            span(message.as_str(), Tone::Error),
            span(format!("{SEPARATOR}press s to retry"), Tone::Plain),
        ],
    };
    let mut facts_rows: Vec<(&str, Vec<Segment>)> = vec![
        (
            "UI",
            vec![linked(
                card.ui_url.as_str(),
                Tone::Link,
                Some(card.ui_url.clone()),
            )],
        ),
        ("Location", vec![span(card.location.as_str(), Tone::Plain)]),
        ("Warehouse", warehouse),
        ("Triggers", vec![span(card.triggers.as_str(), Tone::Plain)]),
        ("Watching", vec![span(card.watching.as_str(), Tone::Plain)]),
    ];
    if let Some(env_file) = &card.env_file {
        facts_rows.push(("Env file", vec![span(env_file.as_str(), Tone::Plain)]));
    }
    let mut rows: Vec<String> = vec![
        render_row(
            vec![
                span(PRODUCT, Tone::Strong),
                span(format!("  v{}   ", card.version), Tone::Muted),
                span(
                    format!("ready in {}", elapsed_text(card.ready_in)),
                    Tone::Strong,
                ),
            ],
            facts,
        ),
        String::new(),
    ];
    for (label, value) in facts_rows {
        let mut segments: Vec<Segment> = vec![
            span(CARD_GLYPH, Tone::Success),
            span(
                format!("  {}", column(label, CARD_LABEL_WIDTH)),
                Tone::Plain,
            ),
        ];
        segments.extend(value);
        rows.push(render_row(segments, facts));
    }
    rows
}

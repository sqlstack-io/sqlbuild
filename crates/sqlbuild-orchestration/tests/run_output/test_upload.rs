//! A worker's output fits the server's batches, long lines are cut between characters, and a refusal never stops later output.

use crate::run_output::constants::{MAX_BATCH_SEGMENTS, MAX_BATCH_TEXT_BYTES};
use crate::run_output::models::{OutputStream, UploadState};
use crate::run_output::tests::helpers::read_at;
use crate::run_output::tests::test_types::{OutputReadTimeTestCase, OutputUploadTestCase};

#[test]
fn given_large_or_mixed_output_when_batched_then_every_batch_fits_the_server_and_nothing_is_lost() {
    let (out, err) = (OutputStream::Stdout, OutputStream::Stderr);
    let lines: Vec<u8> = "order 1234567 shipped to the north warehouse, row checked\n"
        .repeat(9 * 1024 * 1024 / 58)
        .into_bytes();
    let long_line: Vec<u8> = format!("{}\n", "x".repeat(3 * 1024 * 1024)).into_bytes();
    let accents: Vec<u8> = "é".repeat(700_000).into_bytes();
    let test_cases = [
        OutputUploadTestCase {
            description: "one 9 MB write of short lines",
            writes: vec![(out, lines.clone())],
            refused_before: false,
            expected_queued: (3, lines.len(), 0),
        },
        OutputUploadTestCase {
            description: "a 3 MB line is cut into pieces that each end a line",
            writes: vec![(out, long_line.clone())],
            refused_before: false,
            expected_queued: (1, long_line.len() + 2, 0),
        },
        OutputUploadTestCase {
            description: "a 1.4 MB line of two-byte characters with no newline is cut between characters",
            writes: vec![(err, accents.clone())],
            refused_before: false,
            expected_queued: (1, accents.len() + 2, 0),
        },
        OutputUploadTestCase {
            description: "ten thousand writes alternating between the streams",
            writes: (0..10_000)
                .map(|number| ([out, err][number % 2], b"step done\n".to_vec()))
                .collect(),
            refused_before: false,
            expected_queued: (3, 100_000, 0),
        },
        OutputUploadTestCase {
            description: "output after a refused batch is still taken",
            writes: vec![(out, b"after the refusal\n".to_vec())],
            refused_before: true,
            expected_queued: (1, 18, 0),
        },
    ];
    for test_case in test_cases {
        let mut state = UploadState::default();
        state.summary.failure = test_case
            .refused_before
            .then(|| "the control server did not accept run output".to_owned());
        for (stream, bytes) in &test_case.writes {
            state.append(("model:orders".into(), *stream), (bytes, read_at(0)));
        }
        state.end_lines();
        state.seal();
        let too_big: Vec<(usize, usize)> = state
            .queued
            .iter()
            .map(|batch| {
                (
                    batch
                        .segments
                        .iter()
                        .map(|segment| segment.text.len())
                        .sum(),
                    batch.segments.len(),
                )
            })
            .filter(|(bytes, segments)| {
                *bytes > MAX_BATCH_TEXT_BYTES || *segments > MAX_BATCH_SEGMENTS
            })
            .collect();
        assert_eq!(
            too_big,
            Vec::new(),
            "{}: batches past the server's limits",
            test_case.description
        );
        let text: String = state
            .queued
            .iter()
            .flat_map(|batch| batch.segments.iter().map(|segment| segment.text.as_str()))
            .collect();
        assert_eq!(
            (
                state.queued.len(),
                text.len(),
                text.matches('\u{FFFD}').count()
            ),
            test_case.expected_queued,
            "{}: (batches, text bytes, replacement characters)",
            test_case.description
        );
    }
}

#[test]
fn given_lines_read_at_different_times_when_batched_then_each_keeps_the_time_it_was_read() {
    let out = OutputStream::Stdout;
    let test_cases = [
        OutputReadTimeTestCase {
            description: "lines read in one millisecond share a segment",
            writes: vec![(b"one\ntwo\n".to_vec(), 0), (b"three\n".to_vec(), 0)],
            expected_segments: vec![("one\ntwo\nthree\n", 0)],
        },
        OutputReadTimeTestCase {
            description: "a line read later starts a segment with its own time",
            writes: vec![(b"one\n".to_vec(), 0), (b"two\n".to_vec(), 1_250)],
            expected_segments: vec![("one\n", 0), ("two\n", 1_250)],
        },
        OutputReadTimeTestCase {
            description: "a line takes the time the bytes completing it were read",
            writes: vec![(b"on".to_vec(), 0), (b"e\n".to_vec(), 40)],
            expected_segments: vec![("one\n", 40)],
        },
    ];
    for test_case in test_cases {
        let mut state = UploadState::default();
        for (bytes, millis) in &test_case.writes {
            state.append(("model:orders".into(), out), (bytes, read_at(*millis)));
        }
        state.seal();
        let segments: Vec<(String, chrono::DateTime<chrono::Utc>)> = state
            .queued
            .iter()
            .flat_map(|batch| batch.segments.iter())
            .map(|segment| (segment.text.clone(), segment.read_at))
            .collect();
        assert_eq!(
            segments,
            test_case
                .expected_segments
                .iter()
                .map(|(text, millis)| ((*text).to_owned(), read_at(*millis)))
                .collect::<Vec<_>>(),
            "{}",
            test_case.description
        );
    }
}

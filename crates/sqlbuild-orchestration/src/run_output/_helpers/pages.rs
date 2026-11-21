//! Pages of a view's lines with similar lines folded, searches from a line in either direction, and a view's whole text.

use crate::run_output::_helpers::chunks::similar_key;
use crate::run_output::_helpers::lines::segment_text;
use crate::run_output::_helpers::objects::read_chunk;
use crate::run_output::_helpers::views::view_segments;
use crate::run_output::constants::{
    AROUND_OPEN_LINES, COLLAPSE_SCAN_LINES, MIN_SIMILAR_LINES, SEARCH_CHUNKS, SEGMENT_BATCH,
};
use crate::run_output::models::{
    OutputLine, OutputPageQuery, OutputRow, OutputSearch, OutputStore, OutputView, PageRows,
    SearchDirection, SearchTally, SimilarRun, ViewReader,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Up to `limit` rows in the reader's direction, in line order; with `collapse`, runs of similar lines fold, and a run may be read past the last row to its end.
pub(crate) async fn page_rows(
    reader: &mut ViewReader<'_>,
    limit: usize,
    (collapse, around): (bool, Option<i64>),
) -> Result<PageRows, ControlError> {
    let mut page = PageRows::default();
    let mut run = SimilarRun::default();
    let mut scanned_past: i64 = 0;
    while page.rows.len() < limit || !run.lines.is_empty() {
        if page.rows.len() >= limit {
            scanned_past += 1;
            if scanned_past > COLLAPSE_SCAN_LINES {
                break;
            }
        }
        let Some(line) = reader.next_line().await? else {
            break;
        };
        let open: bool = around.is_some_and(|at| (line.number - at).abs() <= AROUND_OPEN_LINES);
        if !collapse || open {
            page.flush(std::mem::take(&mut run));
            if page.rows.len() >= limit {
                break;
            }
            page.push_line(line);
            continue;
        }
        let key: String = similar_key(&line.text);
        if run.key.as_deref() != Some(key.as_str()) {
            page.flush(std::mem::take(&mut run));
            if page.rows.len() >= limit {
                break;
            }
            run.key = Some(key);
        }
        run.lines.push(line);
    }
    page.flush(std::mem::take(&mut run));
    if reader.direction == SearchDirection::Backward {
        page.rows.reverse();
    }
    Ok(page)
}

impl SearchTally {
    /// Count a match at `number`, a search from `from` being where it was asked to look.
    fn count(&mut self, number: i64, from: i64) {
        self.count += 1;
        self.first = self.first.or(Some(number));
        self.last = Some(number);
        self.before += i64::from(number < from);
        if number <= from {
            self.through += 1;
            self.upto = Some(number);
        }
        if number >= from && self.after.is_none() {
            self.after = Some(number);
        }
    }
}

impl PageRows {
    fn push_line(&mut self, line: OutputLine) {
        self.cover((line.number, line.number));
        self.rows.push(OutputRow::Line { line });
    }

    fn cover(&mut self, (low, high): (i64, i64)) {
        self.low = Some(self.low.map_or(low, |seen| seen.min(low)));
        self.high = Some(self.high.map_or(high, |seen| seen.max(high)));
    }

    /// Add a run of similar lines: one folded row when it is long enough, else each line.
    fn flush(&mut self, run: SimilarRun) {
        let lines: Vec<OutputLine> = run.lines;
        if lines.len() < MIN_SIMILAR_LINES {
            for line in lines {
                self.push_line(line);
            }
            return;
        }
        let low: i64 = lines
            .iter()
            .map(|line| line.number)
            .min()
            .unwrap_or_default();
        let high: i64 = lines
            .iter()
            .map(|line| line.number)
            .max()
            .unwrap_or_default();
        let sample: OutputLine = lines
            .iter()
            .find(|line| line.number == low)
            .cloned()
            .unwrap_or_else(|| lines[0].clone());
        self.cover((low, high));
        self.rows.push(OutputRow::Similar {
            from: low,
            count: i64::try_from(lines.len()).unwrap_or(i64::MAX),
            sample,
        });
    }
}

/// Search a small view whole: the match nearest `from` in `direction`, wrapping round, with the count and its rank.
pub(crate) async fn search_whole(
    reader: &mut ViewReader<'_>,
    needle: &str,
    (from, direction): (i64, SearchDirection),
) -> Result<OutputSearch, ControlError> {
    let mut tally = SearchTally::default();
    while let Some(line) = reader.next_line().await? {
        if line.text.to_lowercase().contains(needle) {
            tally.count(line.number, from);
        }
    }
    let count: i64 = tally.count;
    let (line, rank) = match direction {
        SearchDirection::Forward => tally
            .after
            .map_or((tally.first, 1), |found| (Some(found), tally.before + 1)),
        SearchDirection::Backward => tally
            .upto
            .map_or((tally.last, count), |found| (Some(found), tally.through)),
    };
    Ok(OutputSearch {
        line,
        count: Some(count),
        rank: line.map(|_| rank),
        continue_from: None,
    })
}

/// Search a large view from the reader's bound, at most `SEARCH_CHUNKS` chunks: the first match, or where to continue.
pub(crate) async fn search_part(
    reader: &mut ViewReader<'_>,
    needle: &str,
) -> Result<OutputSearch, ControlError> {
    let mut scanned: Option<i64> = None;
    while reader.chunks_read <= SEARCH_CHUNKS {
        let Some(line) = reader.next_line().await? else {
            return Ok(OutputSearch {
                line: None,
                count: None,
                rank: None,
                continue_from: None,
            });
        };
        if line.text.to_lowercase().contains(needle) {
            return Ok(OutputSearch {
                line: Some(line.number),
                count: None,
                rank: None,
                continue_from: None,
            });
        }
        scanned = Some(line.number);
    }
    Ok(OutputSearch {
        line: None,
        count: None,
        rank: None,
        continue_from: scanned.map(|number| match reader.direction {
            SearchDirection::Forward => number + 1,
            SearchDirection::Backward => number - 1,
        }),
    })
}

/// The view's text from `bound` on, one batch of segments, and where the next batch starts; `None` past the end.
pub(crate) async fn text_after(
    (store, output): (&ControlStore, &OutputStore),
    view: &OutputView,
    bound: i64,
) -> Result<Option<(String, i64)>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, segments) = view_segments(
        transaction,
        view,
        (SearchDirection::Forward, bound),
        SEGMENT_BATCH,
    )
    .await?;
    commit(transaction).await?;
    let Some(last) = segments.last() else {
        return Ok(None);
    };
    let next: i64 = last.first_line + last.line_count;
    let mut text: String = String::new();
    for segment in &segments {
        let capacity: usize = usize::try_from(segment.chunk_text_bytes).unwrap_or(usize::MAX);
        let chunk: String = read_chunk(output, &segment.object_key, capacity).await?;
        text.push_str(segment_text(&chunk, segment)?);
    }
    Ok(Some((text, next)))
}

/// Where a page reads from: backwards from the end for `tail`, backwards from `before`, or forwards from `from`; giving more than one is refused.
pub(crate) fn page_start(
    query: &OutputPageQuery,
    line_count: i64,
) -> Result<(SearchDirection, i64), ControlError> {
    let named: usize = [query.from.is_some(), query.before.is_some(), query.tail]
        .into_iter()
        .filter(|named| *named)
        .count();
    if named > 1 {
        return Err(ControlError::Invalid(
            "read a page from one place: from, before or tail".into(),
        ));
    }
    Ok(match (query.tail, query.before) {
        (true, _) => (SearchDirection::Backward, line_count + 1),
        (false, Some(before)) => (SearchDirection::Backward, before),
        (false, None) => (SearchDirection::Forward, query.from.unwrap_or(1).max(1)),
    })
}

/// `from` for the page after lines `from`..`to`, and `before` for the page before them, where the view has such lines.
pub(crate) async fn page_neighbours(
    store: &ControlStore,
    view: &OutputView,
    (from, to): (i64, i64),
) -> Result<(Option<i64>, Option<i64>), ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, later) =
        view_segments(transaction, view, (SearchDirection::Forward, to), 1).await?;
    let (transaction, earlier) =
        view_segments(transaction, view, (SearchDirection::Backward, from), 1).await?;
    commit(transaction).await?;
    Ok((
        (!later.is_empty()).then_some(to),
        (!earlier.is_empty()).then_some(from),
    ))
}

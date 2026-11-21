//! A view's lines in order, forwards or backwards, read a batch of segments at a time.

use std::sync::Arc;

use crate::run_output::_helpers::chunks::{level_of, lines};
use crate::run_output::_helpers::objects::read_chunk;
use crate::run_output::_helpers::views::{view_gap, view_segments};
use crate::run_output::constants::SEGMENT_BATCH;
use crate::run_output::models::{
    OutputLine, OutputStore, OutputView, SearchDirection, StoredSegment, ViewReader,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

impl<'a> ViewReader<'a> {
    /// Read `view` from `bound`: forwards from that line, or backwards from just before it.
    pub(crate) async fn open(
        (store, output): (&'a ControlStore, &'a OutputStore),
        view: &'a OutputView,
        (direction, bound): (SearchDirection, i64),
    ) -> Result<Self, ControlError> {
        let transaction = begin(store, TransactionMode::Read).await?;
        let (transaction, gap) = view_gap(transaction, view).await?;
        commit(transaction).await?;
        Ok(Self {
            store,
            output,
            view,
            direction,
            bound,
            gap,
            pending: std::collections::VecDeque::new(),
            exhausted: false,
            cached: None,
            chunks_read: 0,
        })
    }

    /// The next line in the reader's direction, or `None` past the view's end.
    pub(crate) async fn next_line(&mut self) -> Result<Option<OutputLine>, ControlError> {
        while self.pending.is_empty() && !self.exhausted {
            self.fill().await?;
        }
        Ok(self.pending.pop_front())
    }

    async fn fill(&mut self) -> Result<(), ControlError> {
        let transaction = begin(self.store, TransactionMode::Read).await?;
        let (transaction, segments) = view_segments(
            transaction,
            self.view,
            (self.direction, self.bound),
            SEGMENT_BATCH,
        )
        .await?;
        commit(transaction).await?;
        self.exhausted = segments.is_empty();
        for segment in segments {
            let chunk: Arc<String> = self.chunk(&segment).await?;
            let found: Vec<OutputLine> = self.segment_lines(&chunk, &segment)?;
            self.pending.extend(found);
            self.bound = match self.direction {
                SearchDirection::Forward => segment.first_line + segment.line_count,
                SearchDirection::Backward => segment.first_line,
            };
        }
        Ok(())
    }

    /// The segment's lines on the reader's side of its bound, in the reader's direction.
    fn segment_lines(
        &self,
        chunk: &str,
        segment: &StoredSegment,
    ) -> Result<Vec<OutputLine>, ControlError> {
        let mut found: Vec<OutputLine> = Vec::new();
        for (number, text) in (segment.first_line..).zip(lines(segment_text(chunk, segment)?)) {
            let wanted: bool = match self.direction {
                SearchDirection::Forward => number >= self.bound,
                SearchDirection::Backward => number < self.bound,
            };
            if !wanted {
                continue;
            }
            found.push(OutputLine {
                number,
                node_key: segment.node_key.clone(),
                stream: segment.stream,
                level: level_of(text),
                text: text.to_owned(),
                omitted_before: match self.gap {
                    Some((first, left_out)) if first == number => left_out,
                    _ => 0,
                },
            });
        }
        if self.direction == SearchDirection::Backward {
            found.reverse();
        }
        Ok(found)
    }

    /// The text of the segment's chunk, read once while consecutive segments share it.
    async fn chunk(&mut self, segment: &StoredSegment) -> Result<Arc<String>, ControlError> {
        if let Some((key, text)) = &self.cached
            && *key == segment.object_key
        {
            return Ok(Arc::clone(text));
        }
        let capacity: usize = usize::try_from(segment.chunk_text_bytes).map_err(|_| invalid())?;
        let text: Arc<String> =
            Arc::new(read_chunk(self.output, &segment.object_key, capacity).await?);
        self.cached = Some((segment.object_key.clone(), Arc::clone(&text)));
        self.chunks_read += 1;
        Ok(text)
    }
}

/// The text of one segment within its chunk's text.
pub(crate) fn segment_text<'a>(
    chunk: &'a str,
    segment: &StoredSegment,
) -> Result<&'a str, ControlError> {
    let start: usize = usize::try_from(segment.text_offset).map_err(|_| invalid())?;
    let end: usize = start + usize::try_from(segment.text_bytes).map_err(|_| invalid())?;
    chunk.get(start..end).ok_or_else(invalid)
}

pub(crate) fn invalid() -> ControlError {
    ControlError::Invalid("run output index points outside its chunk".into())
}

//! Remove the next event to deliver from the bounded priority queue.

use crate::event_export::models::{EventQueue, QueuedEvent};

/// The queue without its next event: the oldest at the highest priority, invocation terminals last, and a queued start of the chosen event's invocation before it.
pub fn take_lifecycle_event<T>(
    mut queue: EventQueue<T>,
) -> (EventQueue<T>, Option<QueuedEvent<T>>) {
    let only_terminals: bool = queue.items.iter().all(|queued| queued.invocation_terminal);
    let selectable = |queued: &QueuedEvent<T>| only_terminals || !queued.invocation_terminal;
    let highest: Option<u8> = queue
        .items
        .iter()
        .filter(|queued| selectable(queued))
        .map(|queued| queued.priority)
        .max();
    let position: Option<usize> = queue
        .items
        .iter()
        .enumerate()
        .filter(|(_, queued)| selectable(queued) && Some(queued.priority) == highest)
        .min_by_key(|(_, queued)| queued.sequence)
        .map(|(position, _)| position);
    let position: Option<usize> = position.map(|position| start_first(&queue, position));
    let next: Option<QueuedEvent<T>> = position.map(|position| queue.items.remove(position));
    (queue, next)
}

/// The position of a queued start of the event at `position`'s invocation, or `position` when none is queued.
fn start_first<T>(queue: &EventQueue<T>, position: usize) -> usize {
    let chosen: &QueuedEvent<T> = &queue.items[position];
    if chosen.invocation_start || chosen.invocation_id.is_none() {
        return position;
    }
    let mut start: Option<(usize, u64)> = None;
    for (index, queued) in queue.items.iter().enumerate() {
        if queued.invocation_start
            && queued.invocation_id == chosen.invocation_id
            && start.is_none_or(|(_, sequence)| queued.sequence < sequence)
        {
            start = Some((index, queued.sequence));
        }
    }
    start.map_or(position, |(index, _)| index)
}

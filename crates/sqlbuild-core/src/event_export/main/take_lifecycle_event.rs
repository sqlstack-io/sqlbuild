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
    let position: Option<usize> = position.map(|position| {
        let chosen = &queue.items[position];
        queue
            .items
            .iter()
            .enumerate()
            .filter(|(_, queued)| {
                queued.invocation_start
                    && !chosen.invocation_start
                    && chosen.invocation_id.is_some()
                    && queued.invocation_id == chosen.invocation_id
            })
            .min_by_key(|(_, queued)| queued.sequence)
            .map_or(position, |(start, _)| start)
    });
    let next: Option<QueuedEvent<T>> = position.map(|position| queue.items.remove(position));
    (queue, next)
}

//! Test-only count of the store statements one task issues.

use std::cell::Cell;

tokio::task_local! {
    /// The statements issued so far by a task that counts them.
    pub(crate) static STATEMENTS: Cell<usize>;
}

/// Count one statement when the current task counts them.
pub(crate) fn count_statement() {
    let _ = STATEMENTS.try_with(|count| count.set(count.get() + 1));
}

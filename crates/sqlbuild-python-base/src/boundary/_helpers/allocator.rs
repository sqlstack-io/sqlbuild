//! Process-wide allocator for the many short-lived syntax trees built across analysis threads.

#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

//! Declared and listed refresh settings of maintained models.

/// How stale a maintained model may get: refreshed for its readers, or within a fixed lag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshLag {
    /// Refreshed when the maintained models reading it need it.
    Downstream,
    /// Refreshed within this many seconds.
    Seconds(u64),
}

/// The refresh settings a maintained model declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaintainedSettings {
    pub lag: RefreshLag,
    /// The declared refresh warehouse; unset keeps whichever the object has.
    pub warehouse: Option<String>,
}

/// A maintained model's refresh settings as the warehouse lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveDynamicTable {
    pub target_lag: String,
    pub warehouse: String,
}

/// One planned model: whether it is maintained, whether this plan replaces it, and what it reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaintainedRead {
    pub name: String,
    pub maintained: bool,
    pub replaced: bool,
    pub reads: Vec<String>,
}

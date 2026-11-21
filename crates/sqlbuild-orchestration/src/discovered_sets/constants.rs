//! Limits and names of discovered sets.

/// The snapshot document that keeps every discovered set.
pub const DISCOVERED_DOCUMENT: &str = "discovered";
/// The longest item ID, in characters.
pub const ITEM_ID_MAX_CHARS: usize = 200;
/// The characters an item ID may use besides ASCII letters and digits.
pub const ITEM_ID_PUNCTUATION: [char; 4] = ['_', '.', ':', '-'];
/// What stands for the item ID in the one node of a set that was never discovered.
pub const PLACEHOLDER_ITEM: &str = "*";
/// The shortest `check_every`, in seconds.
pub const MIN_CHECK_EVERY_SECONDS: u64 = 60;
/// Seconds per `check_every` unit, largest first.
pub const CHECK_EVERY_UNITS: [(char, u64); 4] = [('d', 86_400), ('h', 3_600), ('m', 60), ('s', 1)];
/// The key that makes a location runtime request a publication.
pub const GENERATION_KEY: &str = "generation";
/// The key that makes a location runtime request a `check_every` check.
pub const CHECK_DISCOVERED_KEY: &str = "check_discovered";

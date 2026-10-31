//! ClickHouse store batching, error codes, encoding and SQL markers.

use crate::schema::models::Table;

/// Rows one insert batch takes; ClickHouse prefers few large inserts to many small ones.
pub const BATCH_ROWS: usize = 5_000;
/// Longest wait for a batch to fill, so a long command adds at most one part per table every five seconds. A closing session sends what it holds at once, and heartbeats never wait for it.
pub const BATCH_WINDOW_MS: u64 = 5_000;
/// ClickHouse `READONLY`.
pub const READONLY_CODE: u64 = 164;
/// ClickHouse `DATABASE_ACCESS_DENIED`.
pub const DATABASE_ACCESS_DENIED_CODE: u64 = 291;
/// ClickHouse `ACCESS_DENIED`.
pub const ACCESS_DENIED_CODE: u64 = 497;
/// ClickHouse `AUTHENTICATION_FAILED`.
pub const AUTHENTICATION_FAILED_CODE: u64 = 516;
/// ClickHouse `TIMEOUT_EXCEEDED`.
pub const TIMEOUT_EXCEEDED_CODE: u64 = 159;
/// ClickHouse `TOO_MANY_SIMULTANEOUS_QUERIES`.
pub const TOO_MANY_SIMULTANEOUS_QUERIES_CODE: u64 = 202;
/// ClickHouse `SOCKET_TIMEOUT`.
pub const SOCKET_TIMEOUT_CODE: u64 = 209;
/// ClickHouse `NETWORK_ERROR`.
pub const NETWORK_ERROR_CODE: u64 = 210;
/// ClickHouse `MEMORY_LIMIT_EXCEEDED`.
pub const MEMORY_LIMIT_EXCEEDED_CODE: u64 = 241;
/// ClickHouse `TABLE_IS_READ_ONLY`.
pub const TABLE_IS_READ_ONLY_CODE: u64 = 242;
/// ClickHouse `TOO_MANY_PARTS`.
pub const TOO_MANY_PARTS_CODE: u64 = 252;
/// ClickHouse `UNKNOWN_STATUS_OF_INSERT`.
pub const UNKNOWN_STATUS_OF_INSERT_CODE: u64 = 319;
/// ClickHouse `SYSTEM_ERROR`.
pub const SYSTEM_ERROR_CODE: u64 = 425;
/// Server error codes meaning the user may not create or write the store.
pub const PERMISSION_CODES: [u64; 4] = [
    READONLY_CODE,
    DATABASE_ACCESS_DENIED_CODE,
    ACCESS_DENIED_CODE,
    AUTHENTICATION_FAILED_CODE,
];
/// Server error codes worth retrying with the same rows and deduplication tokens.
pub const TRANSIENT_CODES: [u64; 9] = [
    TIMEOUT_EXCEEDED_CODE,
    TOO_MANY_SIMULTANEOUS_QUERIES_CODE,
    SOCKET_TIMEOUT_CODE,
    NETWORK_ERROR_CODE,
    MEMORY_LIMIT_EXCEEDED_CODE,
    TABLE_IS_READ_ONLY_CODE,
    TOO_MANY_PARTS_CODE,
    UNKNOWN_STATUS_OF_INSERT_CODE,
    SYSTEM_ERROR_CODE,
];
/// UNKNOWN_DATABASE: Recall never creates databases.
pub const UNKNOWN_DATABASE_CODE: u64 = 81;
/// Where a client error message states the server code.
pub(crate) const SERVER_CODE_MARKERS: [&str; 2] = ["Code: ", "code: "];
/// The most fractional digits a `DateTime64` holds.
pub(crate) const MAX_DATETIME64_PRECISION: u64 = 9;
/// The fractional digits of Recall timestamps.
pub(crate) const MICROS_DIGITS: u64 = 6;
/// One decimal digit of timestamp scale.
pub(crate) const DECIMAL_SCALE: i64 = 10;
pub(crate) const VARINT_PAYLOAD_BITS: u32 = 7;
pub(crate) const VARINT_CONTINUE: u8 = 0x80;
pub(crate) const VARINT_MASK: u64 = 0x7f;
pub(crate) const DECIMAL_RADIX: u32 = 10;
/// Tables written without a lookup: unique by construction, or evidence themselves.
pub(crate) const UNCHECKED_TABLES: [Table; 2] = [Table::Meta, Table::Conflicts];
/// `formatDateTime` pattern of Recall timestamp text.
pub(crate) const TIMESTAMP_PATTERN: &str = "%Y-%m-%dT%H:%i:%S.%fZ";
/// Where a `CREATE TABLE` query states its TTL, the clause after it, and the day interval in it.
pub(crate) const TTL_MARKER: &str = " TTL ";
pub(crate) const SETTINGS_MARKER: &str = " SETTINGS ";
pub(crate) const DAY_INTERVAL: &str = "toIntervalDay(";

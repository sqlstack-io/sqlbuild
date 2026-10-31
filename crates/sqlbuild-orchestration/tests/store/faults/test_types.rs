//! Transaction failure cases shared by credential, tick and run contracts.

pub(crate) struct TransactionFault {
    pub(crate) description: &'static str,
    pub(crate) table: &'static str,
    pub(crate) operation: &'static str,
    pub(crate) condition: &'static str,
    pub(crate) sqlite_body: &'static str,
    pub(crate) postgres_body: &'static str,
}

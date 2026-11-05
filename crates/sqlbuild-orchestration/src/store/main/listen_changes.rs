//! Establish schema-isolated PostgreSQL LISTEN before subscribing clients or declaring recovery.

use crate::store::errors::ControlError;

pub(crate) async fn listen_changes(
    pool: &sqlx_postgres::PgPool,
) -> Result<sqlx_postgres::PgListener, ControlError> {
    let channel: String =
        sqlx_core::query_scalar::query_scalar("SELECT 'sqb_changes_' || md5(current_schema())")
            .fetch_one(pool)
            .await?;
    let mut listener = sqlx_postgres::PgListener::connect_with(pool).await?;
    listener.listen(&channel).await?;
    Ok(listener)
}

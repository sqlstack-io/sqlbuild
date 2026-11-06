"""Best-effort fingerprint writes for Python node identities."""

from __future__ import annotations

import hashlib
import logging
from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import Fingerprint, PythonNodeIdentity, write_fingerprint
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter


def try_write_python_node_identity_fingerprint(
    *,
    identity: PythonNodeIdentity | None,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
    schema: str | None,
    database: str | None = None,
    target_name: str | None = None,
    logger: logging.Logger | None = None,
) -> None:
    """Append one Python identity row without affecting execution success."""

    if identity is None:
        return
    resolved_logger: logging.Logger = logger or logging.getLogger("sqlbuild.python_identity")
    if schema is None:
        resolved_logger.warning(
            "python identity fingerprint write skipped for '%s': target schema is missing",
            identity.node_name,
        )
        return
    try:
        _ = write_fingerprint(
            connection=connection,
            execute=adapter.execute,
            database=database,
            schema=schema,
            fingerprint=Fingerprint(
                node_type=identity.node_type,
                node_name=identity.node_name,
                target_database=database,
                target_schema=schema,
                target_name=target_name,
                run_id=run_id,
                definition_hash=identity.definition_hash,
                version_hash=identity.version_hash,
                schema_fingerprint=hashlib.sha256(b"").hexdigest(),
                definition=identity.definition_json,
                metadata_json=identity.metadata_json,
                ts=datetime.now(tz=UTC),
            ),
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            read_columns=adapter.get_columns,
            render_add_columns=adapter.render_add_columns,
            render_create_table_sql=adapter.render_create_fingerprint_table_sql,
            render_create_index_sqls=adapter.render_create_fingerprint_index_sqls,
        )
    except Exception as exc:
        resolved_logger.warning(
            "python identity fingerprint write failed for '%s': %s",
            identity.node_name,
            exc,
        )

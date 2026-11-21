"""Static DAG artifact type declarations."""

from __future__ import annotations

from enum import StrEnum


class NodeKind(StrEnum):
    """Framework-owned kind emitted for a static DAG node or check."""

    SOURCE = "source"
    LOADER = "loader"
    SEED = "seed"
    UDF = "udf"
    TABLE_FN = "table_fn"
    MODEL = "model"
    ASSET = "asset"
    CHECK = "check"
    SQL_TEST = "sql_test"
    AUDIT = "audit"
    SCENARIO = "scenario"
    PYTHON_CHECK = "python_check"

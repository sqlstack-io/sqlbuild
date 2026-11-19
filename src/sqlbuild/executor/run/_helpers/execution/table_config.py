"""Adapter config for the tables a model's query is materialized into."""

from __future__ import annotations

from sqlbuild._native import ModelPlanEntry
from sqlbuild.executor.run._helpers.execution.inferred_types import inferred_column_types


def model_output_config(entry: ModelPlanEntry) -> dict[str, object]:
    """The model name, its inferred column types and the output columns analysis expects."""

    return {
        "model_name": entry.name,
        "expected_output_columns": entry.expected_output_columns,
        "inferred_column_types": inferred_column_types(entry),
    }


def model_table_config(entry: ModelPlanEntry) -> dict[str, object]:
    """The model's table type, physical layout, change tracking and expected output columns."""

    return {
        "table_type": entry.table_type,
        "change_tracking": entry.change_tracking,
        "physical_storage": entry.physical_storage,
        **model_output_config(entry),
    }

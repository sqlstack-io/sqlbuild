"""Helpers for public SQLBuild decorator tests."""

from __future__ import annotations


def upstream_asset(_ctx: object) -> dict[str, object]:
    return {"uri": "s3://exports/upstream.parquet"}

"""Shared readers for the custom-rule SQL document tests."""

from __future__ import annotations

from collections.abc import Collection

from sqlbuild.rules import SqlTreeNode


def described_children(node: SqlTreeNode) -> tuple[str, ...]:
    """Return each direct child of ``node`` as ``kind line:column``, in tree order."""

    return tuple(f"{child.kind} {child.line}:{child.column}" for child in node.children)


def clause_children(root: SqlTreeNode, kinds: Collection[str]) -> dict[str, tuple[str, ...]]:
    """Return the one node of each of ``kinds`` under ``root``, with its children."""

    by_kind: dict[str, tuple[str, ...]] = {
        node.kind: described_children(node) for node in root.walk()
    }
    return {kind: by_kind[kind] for kind in kinds}

"""Constants for the Rivers integration."""

from __future__ import annotations

from sqlbuild._native import MaterializationType
from sqlbuild.compiler.dag.types import NodeKind

RIVERS_DEPLOYMENT_ENVIRONMENT_VARIABLE: str = "RIVERS_DEPLOYMENT"
RIVERS_DEVELOPMENT_DEPLOYMENT: str = "dev"
RIVERS_MODEL_KIND: str = NodeKind.MODEL.value
RIVERS_VIEW_MATERIALIZATION: str = MaterializationType.VIEW
RIVERS_ASSET_NODE_KIND_MEMBERS: frozenset[NodeKind] = frozenset(
    {
        NodeKind.SOURCE,
        NodeKind.LOADER,
        NodeKind.SEED,
        NodeKind.MODEL,
        NodeKind.UDF,
        NodeKind.TABLE_FN,
        NodeKind.ASSET,
    }
)
RIVERS_ASSET_NODE_KIND_EXCLUSIONS: frozenset[NodeKind] = frozenset(
    {NodeKind.CHECK, NodeKind.SQL_TEST, NodeKind.AUDIT, NodeKind.SCENARIO, NodeKind.PYTHON_CHECK}
)
RIVERS_ASSET_NODE_KINDS: frozenset[str] = frozenset(
    kind.value for kind in RIVERS_ASSET_NODE_KIND_MEMBERS
)
RIVERS_DIRECT_ASSET_KIND_MEMBERS: frozenset[NodeKind] = frozenset(
    RIVERS_ASSET_NODE_KIND_MEMBERS - {NodeKind.MODEL}
)
RIVERS_DIRECT_ASSET_KIND_EXCLUSIONS: frozenset[NodeKind] = frozenset(
    RIVERS_ASSET_NODE_KIND_EXCLUSIONS | {NodeKind.MODEL}
)
RIVERS_DIRECT_ASSET_KINDS: frozenset[str] = frozenset(
    kind.value for kind in RIVERS_DIRECT_ASSET_KIND_MEMBERS
)

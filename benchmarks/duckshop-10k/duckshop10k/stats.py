"""Shape statistics of a generated project, read back from its files."""

from __future__ import annotations

import re
from collections import Counter
from dataclasses import dataclass
from functools import cache

_REF = re.compile(r'__ref\("([a-z0-9_]+)"\)')
_SOURCE = re.compile(r'__source\("([a-z0-9_]+)"\)')
_TABLE_FN = re.compile(r'__table_fn\("([a-z0-9_]+)"\)')
_TABLE_FN_READS = {"table_fn__customer_orders": "fct_orders"}

LAYERS = ("staging", "intermediate", "marts", "snapshots")


@dataclass(frozen=True)
class Shape:
    models: int
    layers: dict[str, int]
    sources: int
    unit_tests: int
    scenarios: int
    upstream_counts: Counter[int]
    depth: Counter[int]

    @property
    def max_depth(self) -> int:
        return max(self.depth)

    def layer_percent(self, layer: str) -> float:
        return 100 * self.layers.get(layer, 0) / self.models


def _models(files: dict[str, str]) -> dict[str, tuple[str, str]]:
    return {
        path.rsplit("/", 1)[1].removesuffix(".sql"): (path.split("/")[1], text)
        for path, text in files.items()
        if path.startswith("sql/models/") and "/_sqlbuild/" not in path and path.endswith(".sql")
    }


def shape(files: dict[str, str]) -> Shape:
    models = _models(files)
    upstream: dict[str, set[str]] = {}
    for name, (_layer, text) in models.items():
        reads = set(_REF.findall(text))
        reads |= {_TABLE_FN_READS[fn] for fn in _TABLE_FN.findall(text)}
        upstream[name] = reads
    sources = {source for _layer, text in models.values() for source in _SOURCE.findall(text)}

    @cache
    def depth(name: str) -> int:
        parents = upstream[name]
        return 1 + max((depth(parent) for parent in parents), default=0)

    return Shape(
        models=len(models),
        layers=dict(Counter(layer for layer, _text in models.values())),
        sources=len(sources),
        unit_tests=sum(
            text.count("TEST (")
            for path, text in files.items()
            if path.startswith("tests/unit/") and path.endswith(".sql")
        ),
        scenarios=sum(1 for path in files if path.startswith("tests/scenarios/")),
        upstream_counts=Counter(len(parents) for parents in upstream.values()),
        depth=Counter(depth(name) for name in models),
    )

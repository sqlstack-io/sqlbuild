"""Row count estimates of named relations from one catalog query's rows."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any

from sqlbuild.adapter.contract.models import RelationRowCountEstimate


def estimates_by_name(
    *,
    rows: Sequence[Sequence[Any]],
    names: tuple[str, ...],
    estimate: Callable[[tuple[Any, ...] | None], RelationRowCountEstimate],
) -> tuple[RelationRowCountEstimate, ...]:
    """Each name's estimate, in order, from the row leading with that name, or from `None`."""

    by_name: dict[str, tuple[Any, ...]] = {str(row[0]): tuple(row[1:]) for row in rows}
    return tuple(estimate(by_name.get(name)) for name in names)

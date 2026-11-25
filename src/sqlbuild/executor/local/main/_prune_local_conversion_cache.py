"""Drop the project's kept conversions that no run has used for a week."""

from __future__ import annotations

import time

from sqlbuild.executor.local._helpers.conversion import prune_conversion_cache


def prune_local_conversion_cache() -> int:
    """At the end of a local run: record which conversions it used and prune stale ones."""

    return prune_conversion_cache(now=time.time())

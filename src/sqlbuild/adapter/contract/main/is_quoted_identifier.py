"""Column names declared as quoted identifiers, such as `"listingId"`."""

from __future__ import annotations

from sqlbuild.adapter.contract.constants import (
    MIN_QUOTED_IDENTIFIER_LENGTH,
    QUOTED_IDENTIFIER_DELIMITER,
)


def is_quoted_identifier(name: str) -> bool:
    """Whether ``name`` is one double-quoted identifier that keeps its exact spelling."""

    return (
        len(name) >= MIN_QUOTED_IDENTIFIER_LENGTH
        and name.startswith(QUOTED_IDENTIFIER_DELIMITER)
        and name.endswith(QUOTED_IDENTIFIER_DELIMITER)
        and QUOTED_IDENTIFIER_DELIMITER not in name[1:-1]
    )

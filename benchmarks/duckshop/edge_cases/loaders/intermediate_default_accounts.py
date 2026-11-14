"""The loader-dependencies example from the loaders docs, as written.

raw_accounts is an intermediate loader without a write_strategy that returns rows, so it writes
them to __loader__raw_accounts as a table.
"""

from sqlbuild.executor.load.models import LoaderContext
from sqlbuild.loaders import loader


@loader
def raw_accounts(ctx: LoaderContext) -> list[dict[str, object]]:
    """Load customer accounts."""
    return [
        {"account_id": 1, "account_name": "Pawnee Parks"},
        {"account_id": 2, "account_name": "Eagleton"},
    ]


@loader(depends_on=[raw_accounts])
def raw_account_metrics(ctx: LoaderContext) -> list[dict[str, object]]:
    """Account metrics."""
    accounts = ctx.loader(raw_accounts)
    rows = ctx.query(f"SELECT account_id FROM {accounts.destination}")
    return [{"account_id": row[0], "metric": "active"} for row in rows.fetchall()]

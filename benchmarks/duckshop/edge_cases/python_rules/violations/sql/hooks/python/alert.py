"""Alert on failed builds."""


def alert(ctx, reason: str) -> None:
    """Alert on one failed build."""
    print(ctx, reason)

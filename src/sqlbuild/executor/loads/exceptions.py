"""Why a load can't run."""


class LoadRunError(RuntimeError):
    """A load's plan leaves out tables its sources need, or SQLLoad couldn't run it."""

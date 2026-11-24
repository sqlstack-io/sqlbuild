"""Provider exceptions."""


class ProviderInputError(ValueError):
    """Raised when a provider definition is invalid."""


class ProviderLookupError(LookupError):
    """Raised when a provider cannot be found at runtime."""


class ProviderSetupError(RuntimeError):
    """Raised when provider setup fails."""


class ProviderTeardownError(RuntimeError):
    """Raised when provider teardown fails; ``failures`` names each provider and error type."""

    failures: tuple[str, ...] = ()


class ProviderInjectionError(RuntimeError):
    """Raised when a provider cannot be injected into a Python node call."""

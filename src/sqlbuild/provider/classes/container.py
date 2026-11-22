"""Runtime provider container."""

from __future__ import annotations

from collections.abc import Iterator
from typing import TYPE_CHECKING

from sqlbuild.provider.exceptions import ProviderLookupError

if TYPE_CHECKING:
    from sqlbuild.provider.classes.provider import Provider
    from sqlbuild.provider.classes.session import ProviderSession


class ProviderContainer:
    """Name-based runtime provider access."""

    def __init__(self, session: ProviderSession) -> None:
        self._session: ProviderSession = session

    def __getitem__(self, name: str) -> Provider:
        return self._session.get(name)

    def __getattr__(self, name: str) -> Provider:
        if name.startswith("_"):
            raise AttributeError(name)
        try:
            return self._session.get(name)
        except ProviderLookupError as error:
            raise AttributeError(str(error)) from error

    def __contains__(self, name: object) -> bool:
        return isinstance(name, str) and name in self._session

    def __iter__(self) -> Iterator[str]:
        return iter(self.keys())

    def leave_open(self) -> None:
        """Skip teardown, as a step that ignored a cancel may still use the providers."""

        self._session.hold_open()

    def keys(self) -> tuple[str, ...]:
        return self._session.keys()

    def items(self) -> tuple[tuple[str, Provider], ...]:
        return tuple((name, self[name]) for name in self.keys())

"""Helpers for building runtime provider sessions."""

from __future__ import annotations

from typing import TYPE_CHECKING

from sqlbuild._native import DiscoveredProvider
from sqlbuild.provider.classes.session import ProviderSession

if TYPE_CHECKING:
    from sqlbuild._native import EventExporterCommandScope


def build_provider_session(
    *,
    discovered_providers: tuple[DiscoveredProvider, ...],
    setup_context: object | None = None,
    allow_shared: bool = True,
) -> ProviderSession:
    """Build a runtime provider session that reads each provider's settings on first use."""

    if allow_shared:
        from sqlbuild._native import current_event_exporter_command_scope

        scope: EventExporterCommandScope | None = current_event_exporter_command_scope()
        if scope is not None and scope.provider_session is not None:
            expected_names: frozenset[str] = frozenset(
                provider.name for provider in discovered_providers
            )
            if frozenset(scope.provider_session.keys()) == expected_names:
                return scope.provider_session
    return ProviderSession(
        {},
        setup_context=setup_context,
        deferred={
            discovered_provider.name: discovered_provider.provider_class
            for discovered_provider in discovered_providers
        },
    )

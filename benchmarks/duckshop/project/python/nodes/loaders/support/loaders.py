"""Loaders for the synthetic support source tables, generated in python.loaders._support."""

from python.nodes.loaders._registry import load

from sqlbuild.executor.load.models import LoaderContext
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


@factory
def support_loaders() -> list[object]:
    @loader
    def support__tickets(ctx: LoaderContext) -> object:
        return load(ctx, "support__tickets")

    @loader
    def support__ticket_comments(ctx: LoaderContext) -> object:
        return load(ctx, "support__ticket_comments")

    @loader
    def support__csat_responses(ctx: LoaderContext) -> object:
        return load(ctx, "support__csat_responses")

    @loader
    def support__agents(ctx: LoaderContext) -> object:
        return load(ctx, "support__agents")

    @loader
    def support__agent_shifts(ctx: LoaderContext) -> object:
        return load(ctx, "support__agent_shifts")

    @loader
    def support__chat_transcripts(ctx: LoaderContext) -> object:
        return load(ctx, "support__chat_transcripts")

    @loader
    def support__call_records(ctx: LoaderContext) -> object:
        return load(ctx, "support__call_records")

    @loader
    def support__sla_policies(ctx: LoaderContext) -> object:
        return load(ctx, "support__sla_policies")

    return [
        support__tickets,
        support__ticket_comments,
        support__csat_responses,
        support__agents,
        support__agent_shifts,
        support__chat_transcripts,
        support__call_records,
        support__sla_policies,
    ]

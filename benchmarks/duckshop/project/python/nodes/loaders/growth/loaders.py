"""Loaders for the synthetic growth source tables, generated in python.loaders._growth."""

from python.nodes.loaders._registry import load

from sqlbuild.factories import factory
from sqlbuild.executor.load.models import LoaderContext
from sqlbuild.loaders import loader



@factory
def growth_loaders() -> list[object]:
    @loader
    def growth__campaigns(ctx: LoaderContext) -> object:
        return load(ctx, "growth__campaigns")

    @loader
    def growth__campaign_spend(ctx: LoaderContext) -> object:
        return load(ctx, "growth__campaign_spend")

    @loader
    def growth__affiliate_conversions(ctx: LoaderContext) -> object:
        return load(ctx, "growth__affiliate_conversions")

    @loader
    def growth__referral_invites(ctx: LoaderContext) -> object:
        return load(ctx, "growth__referral_invites")

    @loader
    def growth__newsletter_subscriptions(ctx: LoaderContext) -> object:
        return load(ctx, "growth__newsletter_subscriptions")

    @loader
    def growth__survey_responses(ctx: LoaderContext) -> object:
        return load(ctx, "growth__survey_responses")

    @loader
    def growth__influencer_posts(ctx: LoaderContext) -> object:
        return load(ctx, "growth__influencer_posts")


    return [growth__campaigns, growth__campaign_spend, growth__affiliate_conversions, growth__referral_invites, growth__newsletter_subscriptions, growth__survey_responses, growth__influencer_posts]

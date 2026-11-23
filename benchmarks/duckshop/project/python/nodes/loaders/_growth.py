"""Growth change logs: campaigns, spend, affiliates, referrals, newsletter, surveys and creators."""

from __future__ import annotations

from datetime import date, timedelta

from python.nodes.loaders._core import ChangeLog, at, chance, emit, in_zone, random_instant, rng
from python.nodes.loaders._world import CAMPAIGNS, STOREFRONTS, World

Rows = list[dict[str, object]]
SPEND_CURRENCY = {
    "paid_search": "USD",
    "paid_social": "EUR",
    "email": "GBP",
    "influencer": "GBP",
    "affiliate": "GBP",
}
AFFILIATES = ["aff_blog_01", "AFF_REVIEWS_02", "aff-weekly-03", "aff_giftguide_04"]


def campaigns(world: World) -> Rows:
    r = rng("growth__campaigns")
    log = ChangeLog("growth__campaigns")
    for campaign_id, name, channel, starts, ends, budget, _ in CAMPAIGNS:
        targeting: dict[str, object] | None = r.choice(
            [
                {"regions": ["EU", "US"], "age_band": "25-54"},
                {"regions": ["EU", "APAC", "LATAM"], "age_band": "18+"},
                {"age_band": "35-"},
                None,
            ]
        )
        row = {
            "campaign_id": campaign_id,
            "campaign_name": f" {name} " if chance(r, 0.2) else name,
            "channel": channel,
            "objective": r.choice(["Conversions", "awareness", "TRAFFIC"]),
            "starts_on": starts,
            "ends_on": ends,
            "budget_minor": budget,
            "currency_code": "gbp" if chance(r, 0.2) else "GBP",
            "targeting": targeting,
        }
        versions = [(at(starts) - timedelta(days=10), row)]
        if campaign_id == "SPRING-SPLASH":
            versions.append((at(date(2026, 3, 15)), {**row, "budget_minor": budget + 100000}))
        emit(log, r, versions)
    return log.rows()


def campaign_spend(world: World) -> Rows:
    r = rng("growth__campaign_spend")
    log = ChangeLog("growth__campaign_spend")
    count = 0
    for campaign_id, _, channel, starts, ends, budget, _ in CAMPAIGNS:
        if channel in ("email", "affiliate"):
            continue
        day = max(starts, date(2026, 1, 1))
        last = min(ends or date(2026, 3, 31), date(2026, 3, 31))
        daily = budget / max((last - day).days + 1, 1) / 100
        while day <= last:
            count += 1
            spend = round(daily * r.uniform(0.6, 1.3), 2)
            impressions = int(spend * r.randint(80, 200))
            clicks = int(impressions * r.uniform(0.005, 0.03))
            metrics: list[dict[str, object]] = [
                {"name": "spend", "value": f"{spend:,.2f}" if chance(r, 0.4) else spend},
                {"name": "impressions", "value": impressions},
                {"name": "linkClicks" if channel == "paid_social" else "clicks", "value": clicks},
                {"name": "conversions", "value": round(clicks * r.uniform(0.01, 0.05), 3)},
            ]
            if chance(r, 0.1):
                metrics.append({"name": "videoViews", "value": str(r.randint(10, 900))})
            if chance(r, 0.02):
                metrics[0]["value"] = "n/a"
            row = {
                "spend_id": f"SP{count:05d}",
                "spend_date": day,
                "channel": channel,
                "campaign_id": campaign_id if chance(r, 0.97) else None,
                "currency_code": SPEND_CURRENCY[channel],
                "report": {
                    "account": {"id": r.choice([123456789, "act_9876"])},
                    "metrics": metrics,
                },
            }
            versions = [(at(day + timedelta(days=1), 3), row)]
            if chance(r, 0.1):
                # Ad platforms restate spend a few days later.
                restated = [dict(m) for m in metrics]
                restated[0]["value"] = round(spend * 1.02, 2)
                versions.append(
                    (
                        at(day + timedelta(days=4), 3),
                        {**row, "report": {**row["report"], "metrics": restated}},
                    )
                )  # type: ignore[dict-item]
            emit(log, r, versions)
            day += timedelta(days=1)
    return log.rows()


def affiliate_conversions(world: World) -> Rows:
    r = rng("growth__affiliate_conversions")
    log = ChangeLog("growth__affiliate_conversions")
    count = 0
    for order in world.orders:
        if (
            order.utm is None
            or order.utm[1] != "affiliate"
            or order.status in ("pending", "cancelled")
        ):
            continue
        count += 1
        clicked = order.placed_at - timedelta(hours=r.randint(0, 200), minutes=r.randint(0, 59))
        sf = STOREFRONTS[order.storefront]
        row = {
            "conversion_id": f"AF{count:05d}",
            "affiliate_code": r.choice(AFFILIATES),
            "order_id": order.order_id,
            "commission_minor": int(order.net_minor * 0.08),
            "currency_code": sf.currency.lower() if chance(r, 0.2) else sf.currency,
            "converted_at": in_zone(order.placed_at, sf.timezone),
            "click_payload": {
                "clicked_at": in_zone(clicked, "UTC").isoformat() if chance(r, 0.95) else "unknown",
                "landing_url": "https://www.duckshop.example.com/?utm_source=partner_site"
                f"&subid={r.choice(['abc123', 'spring', 'x'])}",
            },
        }
        versions = [(order.placed_at, row)]
        if order.status == "returned":
            versions.append((order.time_of("returned"), {**row, "commission_minor": 0}))  # type: ignore[list-item]
        emit(log, r, versions)
    return log.rows()


def referral_invites(world: World) -> Rows:
    r = rng("growth__referral_invites")
    log = ChangeLog("growth__referral_invites")
    customers = [c for c in world.customers if not c.deleted and c.email]
    for i in range(1, 90 * world.scale + 1):
        referrer = r.choice(customers)
        sent = random_instant(r, date(2026, 1, 1), date(2026, 3, 25))
        invitee = r.choice(customers) if chance(r, 0.4) else None
        email = invitee.email if invitee else f"friend{i}@example.com"
        accepted = None
        if invitee is not None and invitee.created_at > sent:
            accepted = invitee.created_at
        elif chance(r, 0.2):
            accepted = sent + timedelta(days=r.randint(1, 5))
        sf = STOREFRONTS[referrer.storefront]
        row = {
            "invite_id": f"RI{i:05d}",
            "referrer_customer_id": referrer.customer_id,
            "invitee_email": email,
            "sent_at": in_zone(sent, sf.timezone),
            "accepted_at": None,
            "reward": {"amount_minor": 10 * 10**sf.exponent, "currency": sf.currency.lower()}
            if chance(r, 0.9)
            else None,
        }
        versions = [(sent, row)]
        if accepted is not None and accepted.date() <= date(2026, 3, 31):
            versions.append((accepted, {**row, "accepted_at": in_zone(accepted, sf.timezone)}))
        emit(log, r, versions)
    return log.rows()


def newsletter_subscriptions(world: World) -> Rows:
    r = rng("growth__newsletter_subscriptions")
    log = ChangeLog("growth__newsletter_subscriptions")
    count = 0
    pool = [c.email for c in world.customers if c.email] + [
        f"reader{i}@example.net" for i in range(60 * world.scale)
    ]
    for email in pool:
        if not chance(r, 0.45):
            continue
        for list_code in r.sample(["weekly", "offers", "collectors"], k=r.randint(1, 2)):
            count += 1
            subscribed = random_instant(r, date(2025, 6, 1), date(2026, 3, 25))
            row = {
                "list_subscription_id": f"NL{count:06d}",
                "email": email,
                "list_code": list_code,
                "list_status": r.choice(["pending", "Subscribed"]),
                "subscribed_at": in_zone(subscribed, "UTC"),
                "unsubscribed_at": None,
                "signup_source": r.choice(
                    [
                        {"form": "footer", "double_opt_in": True},
                        {"form": " "},
                        {"form": "checkout", "double_opt_in": "false"},
                        None,
                    ]
                ),
            }
            versions = [(subscribed, row)]
            if row["list_status"] == "pending":
                versions.append(
                    (
                        subscribed + timedelta(hours=r.randint(1, 48)),
                        {**row, "list_status": "subscribed"},
                    )
                )
            if chance(r, 0.12):
                gone = subscribed + timedelta(days=r.randint(3, 120))
                if gone.date() <= date(2026, 3, 31):
                    versions.append(
                        (
                            gone,
                            {
                                **versions[-1][1],
                                "list_status": r.choice(["unsubscribed", "CLEANED"]),
                                "unsubscribed_at": in_zone(gone, "UTC"),
                            },
                        )
                    )
            emit(log, r, versions)
    return log.rows()


def survey_responses(world: World) -> Rows:
    r = rng("growth__survey_responses")
    log = ChangeLog("growth__survey_responses")
    count = 0
    for order in world.orders:
        delivered = order.time_of("delivered")
        if delivered is None or not chance(r, 0.12):
            continue
        count += 1
        when = delivered + timedelta(days=r.randint(1, 7))
        if when.date() > date(2026, 3, 31):
            continue
        answers: dict[str, object] = {
            "nps": r.choice([10, 9, "8", 7, 3, "11", None, "n/a"]),
            "delivery_rating": r.choice([5, 4, 2]),
            "comment": r.choice(["Lovely ducks", "", None, "Box was soggy"]),
        }
        if chance(r, 0.3):
            answers["would_buy_again"] = r.choice([True, "yes"])
        row = {
            "response_id": f"SV{count:05d}",
            "survey_code": r.choice(["post-delivery", "NPS-2026Q1"]),
            "customer_id": order.customer_id,
            "submitted_at": in_zone(when, STOREFRONTS[order.storefront].timezone),
            "answers": answers,
        }
        emit(log, r, [(when, row)])
    return log.rows()


def influencer_posts(world: World) -> Rows:
    r = rng("growth__influencer_posts")
    log = ChangeLog("growth__influencer_posts")
    count = 0
    for handle in ["@creator_001", "creator_002", "@Creator_003", "@creator_004"]:
        for _ in range(r.randint(3, 8)):
            count += 1
            posted = random_instant(r, date(2026, 3, 1), date(2026, 3, 30))
            impressions = r.randint(0, 80000)
            metrics: dict[str, object] | None = {
                "impressions": impressions,
                "likes": int(impressions * r.uniform(0.01, 0.08)),
                "comments": r.randint(0, 300),
                "shares": r.choice([None, r.randint(0, 120)]),
            }
            if chance(r, 0.05):
                metrics = None
            row = {
                "post_id": f"IP{count:04d}",
                "creator_handle": handle,
                "platform": r.choice(["video_app", "PHOTO_APP", "blog"]),
                "posted_at": in_zone(posted, "Europe/London"),
                "promo_code": r.choice(["GOLDENEGG", " goldenegg ", None]),
                "metrics": metrics,
            }
            versions = [(posted, row)]
            if metrics is not None and chance(r, 0.5):
                later = posted + timedelta(days=2)
                versions.append(
                    (later, {**row, "metrics": {**metrics, "impressions": impressions * 2}})
                )
            emit(log, r, versions)
    return log.rows()


TABLES = {
    "growth__campaigns": campaigns,
    "growth__campaign_spend": campaign_spend,
    "growth__affiliate_conversions": affiliate_conversions,
    "growth__referral_invites": referral_invites,
    "growth__newsletter_subscriptions": newsletter_subscriptions,
    "growth__survey_responses": survey_responses,
    "growth__influencer_posts": influencer_posts,
}

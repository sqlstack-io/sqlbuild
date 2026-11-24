"""Clickstream event logs: web and app events, searches, email events and experiment assignments."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timedelta
from functools import lru_cache

from python.nodes.loaders._core import EventLog, at, chance, in_zone, naive_utc, rng
from python.nodes.loaders._world import (
    CAMPAIGNS,
    STOREFRONTS,
    World,
    build_world,
    clickstream_days,
    utm_for,
)

Rows = list[dict[str, object]]
SITE = "https://www.duckshop.example.com"
BROWSER_UA = [
    "Waddle/5.0 (Featheros 3; Webfoot) Paddle/124.0",
    "Waddle/5.0 (Pondphone; Quill OS 17_4) Paddle/124.0 Mobile",
    "Waddle/5.0 (Nestbook; Featheros 14_4) Waddle/17.4",
]
BOT_UA = ["Waddle/5.0 (compatible; pondcrawler/2.1)", "Waddle/5.0 HeadlessPaddle/1.0"]
REFERRERS = [
    None,
    None,
    "https://search.example.com/?q=rubber+duck",
    "https://social.example.net/post/9",
    "https://blog.example.org/best-bath-toys",
    f"{SITE}/",
]
SEARCH_TERMS = [
    "rubber duck",
    "  Mallard   DECOY ",
    "teal towel",
    "duck mug!",
    "pond float",
    "giant duck",
    "eider cushion",
    "golden duck",
    "zzzz",
    "bath-time set",
]


@dataclass
class WebEvent:
    event_id: str
    received_at: datetime
    payload: dict[str, object]
    late: bool = False


@dataclass
class Session:
    anonymous_id: str
    customer_id: str | None
    timezone: str | None
    events: list[WebEvent] = field(default_factory=list)


def _ms(value: datetime) -> int:
    return int((naive_utc(value) - datetime(1970, 1, 1)).total_seconds() * 1000)


@lru_cache(maxsize=4)
def web_sessions(scale: int) -> tuple[Session, ...]:  # noqa: PLR0915 - one session script
    """Every planned web session, with its events in time order."""

    world = build_world(scale)
    r = rng("web_sessions")
    customers = [c for c in world.customers if not c.deleted]
    anonymous_pool = [f"anon-{r.getrandbits(48):012x}" for _ in range(350 * scale)]
    sessions: list[Session] = []
    counter = [0]
    days = clickstream_days()
    web_orders = [
        o for o in world.orders if o.placed_at.date() >= days[0] and o.channel in ("web", None)
    ]

    def event(session: Session, when: datetime, kind: str, **extra: object) -> None:
        counter[0] += 1
        payload: dict[str, object] = {
            "type": kind,
            "ts_ms": _ms(when),
            "anonymous_id": session.anonymous_id
            if chance(r, 0.95)
            else session.anonymous_id.upper(),
            "context": {"ua": session_ua, "locale": session_locale, "tz": session.timezone},
        }
        if session.customer_id is not None and kind in (
            "identify",
            "login",
            "order_completed",
            "page_view",
        ):
            customer = world.customer_by_id[session.customer_id]
            payload["user"] = {"id": session.customer_id, "email": customer.email}
        for key, value in extra.items():
            payload[key] = value
        session.events.append(
            WebEvent(
                f"E{counter[0]:08d}", when + timedelta(milliseconds=r.randint(150, 4000)), payload
            )
        )

    for day in days:
        planned: list[tuple[str, str | None, str | None, datetime, object]] = []
        for order in web_orders:
            if order.placed_at.date() == day:
                customer = world.customer_by_id[order.customer_id]
                start = order.placed_at - timedelta(minutes=r.randint(6, 25))
                planned.append(
                    (
                        customer.anonymous_id,
                        customer.customer_id,
                        STOREFRONTS[customer.storefront].timezone,
                        start,
                        order,
                    )
                )
        for _ in range(45 * scale):
            if chance(r, 0.25):
                customer = r.choice(customers)
                anon, customer_id, zone = (
                    customer.anonymous_id,
                    customer.customer_id,
                    STOREFRONTS[customer.storefront].timezone,
                )
            else:
                anon, customer_id, zone = (
                    r.choice(anonymous_pool),
                    None,
                    r.choice(
                        [
                            "Europe/London",
                            "America/New_York",
                            "Asia/Tokyo",
                            "Australia/Sydney",
                            None,
                        ]
                    ),
                )
            planned.append(
                (anon, customer_id, zone, at(day) + timedelta(seconds=r.randint(0, 86399)), None)
            )
        # Sessions that run past midnight UTC.
        for _ in range(3):
            anon = r.choice(anonymous_pool)
            planned.append(
                (anon, None, "America/New_York", at(day, 23, r.randint(35, 55)), "midnight")
            )
        for anon, customer_id, zone, start, purpose in planned:
            session = Session(anon, customer_id, zone)
            session_ua = (
                r.choice(BOT_UA) if purpose is None and chance(r, 0.04) else r.choice(BROWSER_UA)
            )
            session_locale = r.choice(["en_GB", "en-US", "de_DE", "ja-JP", None])
            when = start
            utm = r.choice(utm_for(start)) if chance(r, 0.5) else None
            if purpose not in (None, "midnight") and getattr(purpose, "utm", None):
                utm = purpose.utm  # type: ignore[union-attr]
            landing = (
                f"{SITE}/"
                if chance(r, 0.4)
                else f"{SITE}/c/{r.choice(['rubber-ducks', 'decoys', 'textiles'])}"
            )
            if utm and utm[0]:
                landing += f"?utm_source={utm[0]}&utm_medium={utm[1]}&utm_campaign={utm[2]}"
            elif utm and utm[1] == "organic":
                landing += "?utm_medium=organic"
            event(
                session,
                when,
                "page_view",
                page={"url": landing, "referrer": r.choice(REFERRERS)},
                properties={"title": "Mallard & Co."},
            )
            if customer_id is not None and chance(r, 0.6):
                when += timedelta(seconds=r.randint(5, 60))
                event(
                    session,
                    when,
                    r.choice(["login", "identify", "Identify"]),
                    properties={"method": r.choice(["password", "magic-link", None])},
                )
            browsed = r.sample(world.variants, k=r.randint(0, 4))
            for position, variant in enumerate(browsed, start=1):
                when += timedelta(seconds=r.randint(10, 240))
                url = f"{SITE}/products/{variant.sku.lower()}"
                event(
                    session,
                    when,
                    "page_view",
                    page={"url": url, "referrer": f"{SITE}/"},
                    properties={"title": f"{variant.species} - Mallard & Co."},
                )
                event(
                    session,
                    when + timedelta(milliseconds=300),
                    r.choice(["product_view", "Product View"]),
                    page={"url": url},
                    properties={
                        "sku": variant.raw_sku,
                        "list": {"name": "category", "position": position},
                    },
                )
            if chance(r, 0.2):
                when += timedelta(seconds=r.randint(5, 90))
                term = r.choice(SEARCH_TERMS)
                event(
                    session,
                    when,
                    "search",
                    page={"url": f"{SITE}/search?q={term.strip()}"},
                    properties={
                        "query": term,
                        "results": 0 if term == "zzzz" else r.choice([3, 12, 40, None]),
                    },
                )
            ordering = purpose not in (None, "midnight")
            if browsed and (ordering or chance(r, 0.35)):
                for variant in browsed[:2]:
                    when += timedelta(seconds=r.randint(5, 120))
                    event(
                        session,
                        when,
                        "add_to_cart",
                        page={"url": f"{SITE}/products/{variant.sku.lower()}"},
                        properties={"currency": r.choice(["gbp", "EUR", "usd"])},
                        items=[
                            {
                                "sku": variant.raw_sku,
                                "quantity": r.choice([1, 1, 2, None]),
                                "price_minor": int(variant.base_usd * 100),
                            }
                        ],
                    )
                if chance(r, 0.15):
                    when += timedelta(seconds=r.randint(5, 60))
                    event(session, when, "remove_from_cart", items=[{"sku": browsed[0].raw_sku}])
                if ordering or chance(r, 0.4):
                    when += timedelta(seconds=r.randint(20, 120))
                    event(
                        session, when, "checkout_started", page={"url": f"{SITE}/checkout/step-1"}
                    )
                    for step, name in ((2, "Delivery"), (3, "Payment")):
                        if not ordering and chance(r, 0.4):
                            break
                        when += timedelta(seconds=r.randint(20, 200))
                        event(
                            session,
                            when,
                            "checkout_step",
                            page={"url": f"{SITE}/checkout/step-{step}"},
                            properties={
                                "step": step,
                                "step_name": name,
                                "payment_option": r.choice(["Card", "phone_wallet", None])
                                if step == 3
                                else None,
                            },
                        )
            if ordering:
                order = purpose
                when = max(when + timedelta(seconds=30), order.placed_at)  # type: ignore[union-attr]
                event(
                    session,
                    when,
                    "order_completed",
                    page={"url": f"{SITE}/checkout/thank-you"},
                    properties={"order_id": order.order_id, "revenue_minor": order.total_minor},
                )  # type: ignore[union-attr]
            if purpose == "midnight":
                for _ in range(r.randint(2, 4)):
                    when += timedelta(minutes=r.randint(6, 20))
                    event(
                        session,
                        when,
                        "page_view",
                        page={"url": f"{SITE}/c/decoys?page={r.randint(2, 5)}"},
                    )
            # A few sessions lose their last event in transit; it arrives a day or two later.
            if len(session.events) > 2 and chance(r, 0.03):
                last = session.events[-1]
                last.received_at = last.received_at + timedelta(
                    days=r.randint(1, 2), hours=r.randint(0, 5)
                )
                last.late = True
            sessions.append(session)
    return tuple(sessions)


def web_events(world: World) -> Rows:
    r = rng("clickstream__web_events")
    log = EventLog("clickstream__web_events")
    for session in web_sessions(world.scale):
        for e in session.events:
            payload = dict(e.payload)
            if chance(r, 0.005):
                payload.pop("ts_ms")
            row = {
                "event_id": e.event_id,
                "received_at": naive_utc(e.received_at),
                "event": payload,
            }
            loaded = e.received_at + timedelta(minutes=r.randint(1, 20))
            log.add(row, loaded)
            if chance(r, 0.015):
                log.add(row, loaded + timedelta(minutes=r.randint(30, 300)))
    return log.rows()


def app_events(world: World) -> Rows:
    r = rng("clickstream__app_events")
    log = EventLog("clickstream__app_events")
    customers = [c for c in world.customers if not c.deleted]
    devices = []
    for i in range(1, 120 * world.scale + 1):
        customer = r.choice(customers) if chance(r, 0.7) else None
        devices.append(
            (
                f"dev-{i:05d}",
                customer.customer_id if customer else None,
                r.choice(["Phone", "phone", "TABLET", "tablet"]),
                r.choice(["3.4.1", "v3.10.0-beta", "4", "4.0.2 (build 77)"]),
            )
        )
    count = 0
    for day in clickstream_days():
        for _ in range(25 * world.scale):
            device_id, customer_id, platform, version = r.choice(devices)
            when = at(day) + timedelta(seconds=r.randint(0, 86399))
            names = ["app_open"] + ["screen_view"] * r.randint(1, 5)
            if chance(r, 0.15):
                names.insert(0, "push_open")
            if chance(r, 0.2):
                names.append("add_to_cart")
            for name in names:
                count += 1
                when += timedelta(seconds=r.randint(2, 300))
                payload: dict[str, object] = {
                    "name": name if chance(r, 0.9) else name.upper(),
                    "device": {"id": device_id},
                    "user_id": customer_id,
                }
                if chance(r, 0.5):
                    payload["ts"] = in_zone(when, "UTC").strftime("%Y-%m-%d %H:%M:%S.%f")
                else:
                    payload["ts_ms"] = _ms(when)
                if name == "screen_view":
                    payload["screen"] = r.choice(
                        ["home", "product_detail", "basket", "order_history"]
                    )
                    payload["attributes"] = {
                        "screen_class": r.choice(["HomeVC", "ProductActivity", None])
                    }
                if name == "push_open":
                    campaign = r.choice([c[0] for c in CAMPAIGNS])
                    payload["attributes"] = r.choice(
                        [
                            {"campaign_id": campaign},
                            {"deep_link": f"duckshop://promo/spring?cid={campaign}"},
                            {"deep_link": "duckshop://home"},
                        ]
                    )
                received = when + timedelta(seconds=r.randint(1, 30))
                if chance(r, 0.02):
                    received += timedelta(hours=r.randint(2, 30))
                row = {
                    "event_id": f"AE{count:08d}",
                    "received_at": received,
                    "platform": platform,
                    "app_version": version,
                    "payload": payload,
                }
                log.add(row, received + timedelta(minutes=r.randint(1, 10)))
                if chance(r, 0.01):
                    log.add(row, received + timedelta(hours=3))
    return log.rows()


def search_queries(world: World) -> Rows:
    r = rng("clickstream__search_queries")
    log = EventLog("clickstream__search_queries")
    anonymous = [s.anonymous_id for s in web_sessions(world.scale)]
    count = 0
    for day in clickstream_days():
        for _ in range(20 * world.scale):
            count += 1
            term = r.choice(SEARCH_TERMS + ["Mallard's decoy (large)", "duck-shaped soap"])
            when = at(day) + timedelta(seconds=r.randint(0, 86399))
            filters: dict[str, object] | None = r.choice(
                [
                    None,
                    {"category": "Decoys", "sort": "price_asc"},
                    {"price": {"max_minor": r.choice([2000, 5000, "9999"])}},
                    {"category": None},
                ]
            )
            row = {
                "query_id": f"Q{count:07d}",
                "anonymous_id": r.choice(anonymous),
                "query_text": term,
                "results_count": 0 if term in ("zzzz", "duck-shaped soap") else r.randint(1, 60),
                "searched_at": in_zone(
                    when, r.choice(["Europe/London", "America/New_York", "Asia/Tokyo"])
                ),
                "filters": filters,
            }
            log.add(row, when + timedelta(minutes=r.randint(1, 30)))
            if chance(r, 0.02):
                log.add(row, when + timedelta(hours=2))
    return log.rows()


EMAIL_TYPES = {"sent": 0, "delivered": 1, "open": 2, "click": 3}


def email_events(world: World) -> Rows:
    r = rng("clickstream__email_events")
    log = EventLog("clickstream__email_events")
    customers = [c for c in world.customers if c.email]
    email_campaigns = [
        ("POND-WEEK-EMAIL", at(clickstream_days()[0]) - timedelta(days=20)),
        ("SPRING-SPLASH", at(clickstream_days()[0], 8)),
        ("GOLDEN-EGG", at(clickstream_days()[0] + timedelta(days=10), 8)),
    ]
    count = 0
    message = 0
    for campaign_id, sent_at in email_campaigns:
        for customer in r.sample(customers, k=min(len(customers), 120 * world.scale)):
            message += 1
            message_id = f"<msg{message:07d}@mail.duckshop.example.com>"
            when = sent_at + timedelta(seconds=r.randint(0, 3600))
            journey = ["sent", "delivered"]
            if chance(r, 0.05):
                journey = ["sent", "bounce"]
            else:
                if chance(r, 0.45):
                    journey.append("open")
                    if chance(r, 0.35):
                        journey.append("click")
                if chance(r, 0.03):
                    journey.append("unsubscribe")
                if chance(r, 0.005):
                    journey.append("complaint")
            for kind in journey:
                count += 1
                when += timedelta(minutes=r.randint(1, 600))
                details: dict[str, object] | None = None
                if kind == "click":
                    details = {
                        "url": f"{SITE}/c/decoys?utm_source=newsletter&utm_medium=email"
                        f"&utm_campaign={campaign_id.lower()}"
                    }
                elif kind == "open":
                    details = r.choice(
                        [
                            {"machine_open": True},
                            {"user_agent": "Waddle/5.0 (via ImageProxy)"},
                            {"user_agent": BROWSER_UA[1]},
                            None,
                        ]
                    )
                elif kind == "bounce":
                    details = {"bounce": {"type": r.choice(["Hard", "soft"])}}
                row = {
                    "email_event_id": f"EM{count:08d}",
                    "message_id": message_id,
                    "event_type": kind if chance(r, 0.9) else kind.upper(),
                    "recipient_email": customer.email,
                    "campaign_id": campaign_id,
                    "occurred_at": in_zone(when, "UTC"),
                    "details": details,
                }
                log.add(row, when + timedelta(minutes=r.randint(1, 120)))
                if chance(r, 0.02):
                    log.add(row, when + timedelta(hours=6))
    return log.rows()


def experiment_assignments(world: World) -> Rows:
    r = rng("clickstream__experiment_assignments")
    log = EventLog("clickstream__experiment_assignments")
    count = 0
    seen: set[tuple[str, str]] = set()
    experiments = [
        ("checkout-button-colour", ["control", "yellow"]),
        ("Free-Shipping-Banner", ["a", "b", "c"]),
    ]
    for session in web_sessions(world.scale):
        if session.customer_id is None and not chance(r, 0.5):
            continue
        key, variants = r.choice(experiments)
        start = (
            session.events[0].payload["ts_ms"]
            if session.events and "ts_ms" in session.events[0].payload
            else None
        )
        if start is None:
            continue
        when = datetime(1970, 1, 1) + timedelta(milliseconds=int(start))  # type: ignore[arg-type]
        count += 1
        variant = r.choice(variants)
        repeat = (key, session.anonymous_id) in seen
        seen.add((key, session.anonymous_id))
        row = {
            "assignment_id": f"XA{count:07d}",
            "experiment_key": key,
            "variant_key": variant.upper() if chance(r, 0.1) else variant,
            "anonymous_id": session.anonymous_id,
            "customer_id": session.customer_id,
            "assigned_at": in_zone(when, session.timezone or "UTC"),
            "context": {"surface": "web", "reassigned": repeat} if chance(r, 0.8) else None,
        }
        log.add(row, when + timedelta(minutes=r.randint(1, 15)))
        if chance(r, 0.02):
            log.add(row, when + timedelta(hours=1))
    return log.rows()


TABLES = {
    "clickstream__web_events": web_events,
    "clickstream__app_events": app_events,
    "clickstream__search_queries": search_queries,
    "clickstream__email_events": email_events,
    "clickstream__experiment_assignments": experiment_assignments,
}

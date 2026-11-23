"""Support change logs: tickets, comments, CSAT, agents, shifts, chats, calls and SLA policies."""

from __future__ import annotations

from datetime import date, datetime, timedelta
from functools import lru_cache

from python.nodes.loaders._core import (
    ChangeLog,
    at,
    chance,
    emit,
    in_zone,
    random_instant,
    rng,
    weighted,
)
from python.nodes.loaders._world import World, build_world

Rows = list[dict[str, object]]
AGENTS = [
    # id, name, team, zone, skills
    ("AG001", "Robin Agent", "tier1", "Europe/Berlin", ["lang:de", "lang:en"]),
    ("AG002", "Sam Agent", "TIER1", "Europe/Berlin", ["lang:en"]),
    ("AG003", "Kim Agent", "tier1", "Europe/London", ["lang:en", "returns"]),
    ("AG004", "Noor Agent", "tier2", "Europe/London", ["lang:en", "lang:ar"]),
    ("AG005", "Yuki Agent", "tier1", "Asia/Tokyo", ["lang:ja", "lang:en"]),
    ("AG006", "Alex Agent", "tier2", "America/New_York", ["lang:en", "billing"]),
    ("AG007", "Jordan Agent", "returns", "America/New_York", ["returns"]),
    ("AG008", "Hana Agent", "tier1", "Australia/Sydney", None),
]
CHANNELS = ["email", "chat", "phone", "web"]
SUBJECTS = [
    (
        "where_is_my_order",
        "Where is my order {order}? Tracking says delivered but nothing arrived.",
    ),
    ("damaged_item", "My duck arrived broken!! The beak is snapped off, order ref {order}"),
    ("refund_status", "Refund for return RT-{n} still not received after 14 days"),
    (
        "change_address",
        "Please change delivery address to 12 Reed Street, call me on +44 20 7946 0958",
    ),
    ("billing", "Charged twice: card ending 4242 on 2026-03-03 and 03/03/2026"),
    ("product_question", "Is the Mallard decoy dishwasher safe? Email me at someone@example.com"),
]


@lru_cache(maxsize=4)
def ticket_plan(scale: int):
    world = build_world(scale)
    r = rng("ticket_plan")
    customers = [c for c in world.customers if c.email and not c.deleted]
    plan = []
    for i in range(1, 260 * scale + 1):
        customer = r.choice(customers)
        orders = [o for o in world.orders if o.customer_id == customer.customer_id]
        order = r.choice(orders) if orders and chance(r, 0.8) else None
        created = random_instant(r, date(2026, 1, 5), date(2026, 3, 31))
        if order is not None and created < order.placed_at:
            created = order.placed_at + timedelta(hours=r.randint(2, 200))
            if created.date() > date(2026, 3, 31):
                created = at(date(2026, 3, 31), 22)
        # Tickets opened around the European clock change and over the weekend.
        if i <= 4:
            created = [
                datetime(2026, 3, 29, 0, 40),
                datetime(2026, 3, 29, 1, 20),
                datetime(2026, 3, 28, 22, 0),
                datetime(2026, 3, 8, 6, 50),
            ][i - 1]
        reason, template = r.choice(SUBJECTS)
        channel = str(weighted(r, {"email": 45, "chat": 30, "phone": 15, "web": 10}))
        priority = weighted(r, {1: 5, 2: 15, 3: 60, 4: 18, None: 1, 0: 1})
        first_response = created + timedelta(minutes=r.randint(3, 60 * 30))
        solved = first_response + timedelta(hours=r.randint(1, 120)) if chance(r, 0.75) else None
        if solved is not None and solved.date() > date(2026, 3, 31):
            solved = None
        if first_response.date() > date(2026, 3, 31) or chance(r, 0.05):
            first_response = None
            solved = None
        plan.append(
            (
                f"TK{i:05d}",
                customer,
                order,
                created,
                reason,
                template,
                channel,
                priority,
                first_response,
                solved,
                r.choice([a[0] for a in AGENTS]),
            )
        )
    return tuple(plan)


def tickets(world: World) -> Rows:
    r = rng("support__tickets")
    log = ChangeLog("support__tickets")
    for (
        ticket_id,
        customer,
        order,
        created,
        reason,
        _,
        channel,
        priority,
        first_response,
        solved,
        _,
    ) in ticket_plan(world.scale):
        custom: dict[str, object] | None = {
            "contact_reason": reason.replace("_", " ").title() if chance(r, 0.3) else reason
        }
        if order is not None:
            if chance(r, 0.6):
                custom["order_id"] = order.order_id
            else:
                custom["order_ref"] = f"Re: order {order.order_id} missing"
        if chance(r, 0.2):
            custom["labels"] = r.choice(["vip, late_delivery", "  Refund  ,damage", "vip"])
        if chance(r, 0.05):
            custom = None
        tags = r.choice([["Late Delivery", "vip"], ["damage"], [], None, ["refund_request", None]])
        row = {
            "ticket_id": ticket_id,
            "customer_email": customer.email,
            "channel": channel if chance(r, 0.85) else channel.upper(),
            "priority": priority,
            "ticket_status": "new",
            "created_at": in_zone(
                created, r.choice(["Europe/Berlin", "America/New_York", "UTC", "Asia/Tokyo"])
            ),
            "solved_at": None,
            "custom_fields": custom,
            "tags": tags,
        }
        versions = [(created, row)]
        if first_response is not None:
            versions.append((first_response, {**row, "ticket_status": "open"}))
            if chance(r, 0.3):
                versions.append(
                    (first_response + timedelta(minutes=5), {**row, "ticket_status": "Pending"})
                )
        if solved is not None:
            versions.append(
                (
                    solved,
                    {
                        **versions[-1][1],
                        "ticket_status": r.choice(["solved", "closed"]),
                        "solved_at": in_zone(solved, "Europe/Berlin"),
                    },
                )
            )
        emit(log, r, versions, late_share=0.05)
    return log.rows()


def ticket_comments(world: World) -> Rows:
    r = rng("support__ticket_comments")
    log = ChangeLog("support__ticket_comments")
    count = 0
    for (
        ticket_id,
        customer,
        order,
        created,
        _,
        template,
        _,
        _,
        first_response,
        solved,
        agent,
    ) in ticket_plan(world.scale):
        body = template.format(
            order=order.order_id if order else "DS-O-000000", n=r.randint(10, 99)
        )
        comments: list[tuple[str, str | None, str, datetime]] = [
            ("customer", customer.customer_id, body, created)
        ]
        if first_response is not None:
            comments.append(
                (
                    "agent",
                    agent,
                    r.choice(
                        [
                            "Thanks for writing in. I checked order {o} and will update you.",
                            "Sorry to hear that! A replacement duck is on its way.",
                            "Could you send a photo of the damage?",
                        ]
                    ).format(o=order.order_id if order else "DS-O-000000"),
                    first_response,
                )
            )
            if chance(r, 0.4):
                reply = first_response + timedelta(hours=r.randint(1, 30))
                comments.append(
                    (
                        "customer",
                        customer.customer_id,
                        r.choice(
                            [
                                "Thanks, sorted!",
                                "Still waiting...",
                                "This is ridiculous, I want a refund",
                            ]
                        ),
                        reply,
                    )
                )
        if solved is not None:
            comments.append(
                ("system", None, "Ticket solved automatically after 72h without reply", solved)
            )
        for author_type, author_id, text, when in comments:
            count += 1
            if when.date() > date(2026, 3, 31):
                continue
            row = {
                "comment_id": f"TC{count:06d}",
                "ticket_id": ticket_id,
                "author_type": author_type if chance(r, 0.9) else author_type.upper(),
                "author_id": author_id,
                "body": text,
                "is_public": None if chance(r, 0.1) else author_type != "system",
                "created_at": in_zone(when, "UTC"),
                "attachments": r.choice([None, [], [{"name": "photo.jpg", "bytes": 182733}]]),
            }
            emit(log, r, [(when, row)])
    return log.rows()


def csat_responses(world: World) -> Rows:
    r = rng("support__csat_responses")
    log = ChangeLog("support__csat_responses")
    count = 0
    for ticket_id, *_, solved, _ in ticket_plan(world.scale):
        if solved is None or not chance(r, 0.4):
            continue
        count += 1
        when = solved + timedelta(hours=r.randint(1, 48))
        if when.date() > date(2026, 3, 31):
            continue
        row = {
            "response_id": f"CR{count:05d}",
            "ticket_id": ticket_id,
            "score": r.choice([5, 5, 4, 3, 2, 1]),
            "comment": r.choice(["", "  Quick and helpful ", None, "Took far too long"]),
            "responded_at": in_zone(when, "UTC"),
            "answers": r.choice(
                [{"effort": "2"}, {"effort": 5, "resolved": True}, {"effort": "n/a"}, None]
            ),
        }
        emit(log, r, [(when, row)])
    return log.rows()


def agents(world: World) -> Rows:
    r = rng("support__agents")
    log = ChangeLog("support__agents")
    for index, (agent_id, name, team, zone, skills) in enumerate(AGENTS, start=1):
        row = {
            "agent_id": agent_id,
            "display_name": name,
            "email": f"{name.split()[0].lower()}.agent@duckshop.example.com"
            if index != 3
            else " KIM.Agent@DuckShop.example.com ",
            "team_code": team,
            "iana_timezone": zone,
            "hired_on": date(2024, 1, 1) + timedelta(days=index * 45),
            "skills": skills,
        }
        versions = [(at(date(2025, 12, 1)), row)]
        if agent_id == "AG002":
            versions.append(
                (
                    at(date(2026, 2, 1)),
                    {**row, "team_code": "tier2", "skills": ["lang:en", "billing"]},
                )
            )
        emit(log, r, versions)
    return log.rows()


def agent_shifts(world: World) -> Rows:
    r = rng("support__agent_shifts")
    log = ChangeLog("support__agent_shifts")
    count = 0
    day = date(2026, 3, 2)
    while day <= date(2026, 3, 31):
        for agent_id, _, _, zone, _ in AGENTS:
            if day.weekday() >= 5 and not chance(r, 0.2):
                continue
            count += 1
            start_hour = r.choice([7, 8, 9, 13])
            start = datetime.combine(day, datetime.min.time()) + timedelta(hours=start_hour)
            row = {
                "shift_id": f"SF{count:06d}",
                "agent_id": agent_id,
                "shift_start_local": start,
                "shift_end_local": start + timedelta(hours=8, minutes=r.choice([0, 30])),
                "iana_timezone": zone,
                "breaks": r.choice(
                    [
                        [{"minutes": 30}, {"minutes": "15"}],
                        [],
                        None,
                        [{"minutes": 45, "paid": False}],
                    ]
                ),
            }
            emit(log, r, [(start - timedelta(days=7), row)])
        day += timedelta(days=1)
    return log.rows()


def chat_transcripts(world: World) -> Rows:
    r = rng("support__chat_transcripts")
    log = ChangeLog("support__chat_transcripts")
    count = 0
    lines = {
        "customer": [
            "Hi, my duck never arrived",
            "This is the worst service, I want to cancel",
            "My number is 07700 900123 if easier",
            "Thanks, that is great!",
            "ok",
        ],
        "agent": [
            "Hello! Let me check that for you.",
            "I appreciate your patience.",
            "Your refund has been sorted.",
        ],
    }
    for ticket_id, _, _, created, _, _, channel, _, _, _, _ in ticket_plan(world.scale):
        if channel != "chat":
            continue
        count += 1
        started = created
        messages = []
        when = started
        for index in range(r.randint(2, 7)):
            role = "customer" if index % 2 == 0 else r.choice(["agent", "Agent", "bot"])
            when += timedelta(seconds=r.randint(10, 400))
            messages.append(
                {
                    "role": role,
                    "ts": in_zone(when, "UTC").isoformat() if chance(r, 0.95) else "garbled",
                    "text": r.choice(lines["customer" if role == "customer" else "agent"]),
                }
            )
        row = {
            "chat_id": f"CH{count:05d}",
            "ticket_id": ticket_id if chance(r, 0.95) else None,
            "started_at": in_zone(started, "UTC"),
            "ended_at": in_zone(when, "UTC"),
            "messages": messages,
        }
        emit(log, r, [(when, row)])
    return log.rows()


def call_records(world: World) -> Rows:
    r = rng("support__call_records")
    log = ChangeLog("support__call_records")
    count = 0
    paths = [
        ["main", "orders", "where_is_my_order", "self_service_complete"],
        ["main", "orders", "agent"],
        ["main", "returns", "main", "returns", "agent"],
        ["main", "billing", "callback_booked"],
        ["main"],
    ]
    for ticket_id, customer, _, created, _, _, channel, _, _, _, agent in ticket_plan(world.scale):
        if channel != "phone":
            continue
        for attempt in range(r.randint(1, 2)):
            count += 1
            path = r.choice(paths)
            when = created + timedelta(hours=attempt * 5)
            if when.date() > date(2026, 3, 31):
                continue
            row = {
                "call_id": f"CL{count:05d}",
                "ticket_id": ticket_id,
                "agent_id": agent if path[-1] == "agent" else None,
                "phone_number": customer.phone,
                "started_at": in_zone(when, "Europe/London"),
                "duration_seconds": r.randint(20, 1800),
                "ivr_path": path,
            }
            emit(log, r, [(when, row)])
    return log.rows()


def sla_policies(world: World) -> Rows:
    r = rng("support__sla_policies")
    log = ChangeLog("support__sla_policies")
    count = 0
    targets = {1: (30, 8), 2: (60, 24), 3: (240, 72), 4: (480, 120)}
    for priority, (response, resolution) in targets.items():
        for channel in CHANNELS:
            count += 1
            row = {
                "policy_id": f"SLA{count:03d}",
                "priority": priority,
                "channel": channel.title() if chance(r, 0.2) else channel,
                "first_response_minutes": response // (4 if channel == "chat" else 1),
                "resolution_hours": resolution,
                "business_hours_only": None if channel == "web" else priority > 1,
                "calendar": {"region": "eu"} if chance(r, 0.7) else None,
            }
            versions = [(at(date(2025, 12, 1)), row)]
            if priority == 2 and channel == "email":
                versions.append((at(date(2026, 3, 1)), {**row, "first_response_minutes": 45}))
            emit(log, r, versions)
    return log.rows()


TABLES = {
    "support__tickets": tickets,
    "support__ticket_comments": ticket_comments,
    "support__csat_responses": csat_responses,
    "support__agents": agents,
    "support__agent_shifts": agent_shifts,
    "support__chat_transcripts": chat_transcripts,
    "support__call_records": call_records,
    "support__sla_policies": sla_policies,
}

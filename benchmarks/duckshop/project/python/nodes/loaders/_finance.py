"""Finance change logs and feeds: invoices, ledger, payouts, tax rates, chargebacks, settlements."""

from __future__ import annotations

import json
from collections import defaultdict
from datetime import date, datetime, timedelta

from python.nodes.loaders._core import ChangeLog, EventLog, at, chance, emit, in_zone, rng
from python.nodes.loaders._storefront import (
    gateway_reference,
    payments_plan,
    provider_for,
    refunds_plan,
)
from python.nodes.loaders._world import STOREFRONTS, World

Rows = list[dict[str, object]]
TAX_RATES = [
    # country, region, category, rate, valid from, valid to
    ("GB", None, "standard", 20.0, date(2011, 1, 4), None),
    ("GB", None, "reduced", 5.0, date(2011, 1, 4), None),
    ("de", None, "Standard", 19.0, date(2007, 1, 1), None),
    ("DE", None, "reduced", 7.0, date(2007, 1, 1), None),
    ("FR", None, "standard", 20.0, date(2014, 1, 1), None),
    ("NL", None, "standard", 21.0, date(2012, 10, 1), None),
    ("JP", None, "standard", 8.0, date(2014, 4, 1), date(2019, 9, 30)),
    ("JP", None, "standard", 10.0, date(2019, 10, 1), None),
    ("AU", None, "standard", 10.0, date(2000, 7, 1), None),
    ("US", "ny", "standard", 8.875, date(2025, 1, 1), date(2026, 2, 28)),
    ("US", "NY", "standard", 8.8754, date(2026, 3, 1), None),
    ("US", "ma", "standard", 6.25, date(2009, 8, 1), None),
    ("CA", "ON", "standard", 13.0, date(2010, 7, 1), None),
    ("KW", None, "standard", 0.0, date(2020, 1, 1), None),
]
TAX_CODE = {"EU": "VAT_STD", "APAC": "GST", "US": "SALES", "MENA": "NONE"}
TAX_RATE = {"EU": 20.0, "US": 0.0, "APAC": 10.0, "MENA": 0.0}


def tax_rates(world: World) -> Rows:
    r = rng("finance__tax_rates")
    log = ChangeLog("finance__tax_rates")
    for index, (country, region, category, rate, start, end) in enumerate(TAX_RATES, start=1):
        row = {
            "tax_rate_id": f"TR{index:03d}",
            "country_code": country,
            "tax_region": region,
            "tax_category": category,
            "rate_pct": rate,
            "valid_from": start,
            "valid_to": end,
        }
        emit(log, r, [(at(date(2025, 12, 1)), row)])
    return log.rows()


def invoices(world: World) -> Rows:
    r = rng("finance__invoices")
    log = ChangeLog("finance__invoices")
    count = 0
    for order in world.orders:
        paid = order.time_of("paid")
        if paid is None:
            continue
        count += 1
        sf = STOREFRONTS[order.storefront]
        net = order.net_minor
        tax = order.tax_minor
        tax_lines: list[dict[str, object]] | None = [
            {
                "code": TAX_CODE[sf.region].lower() if chance(r, 0.3) else TAX_CODE[sf.region],
                "rate_pct": TAX_RATE[sf.region],
                "taxable_minor": net,
                "tax_minor": tax,
            }
        ]
        if chance(r, 0.03):
            tax_lines = None
        elif chance(r, 0.04) and tax:
            tax_lines = [
                {**tax_lines[0], "tax_minor": tax - 1},
                {"code": "rounding", "rate_pct": 0, "taxable_minor": 0, "tax_minor": 1},
            ]
        totals: dict[str, object] = {"net_minor": net, "tax_minor": tax}
        if chance(r, 0.7):
            totals["gross_minor"] = net + tax
        series = {"EU": "EU", "US": "US", "APAC": "APAC", "MENA": "ME"}[sf.region]
        number = f"{series}-2026-{count:06d}"
        row = {
            "invoice_id": f"INV{count:06d}",
            "order_id": order.order_id,
            "invoice_number": number.lower() if chance(r, 0.1) else number,
            "issued_at": in_zone(paid + timedelta(minutes=r.randint(1, 30)), sf.timezone),
            "currency_code": order.currency,
            "totals": totals,
            "tax_lines": tax_lines,
        }
        versions = [(paid, row)]
        if chance(r, 0.03):
            versions.append(
                (paid + timedelta(days=2), {**row, "totals": {**totals, "note": "reissued"}})
            )
        emit(log, r, versions)
    return log.rows()


ACCOUNTS = {
    "cash": "1000",
    "receivables": "1100",
    "inventory": "1300",
    "tax": "2200",
    "gift_cards": "2300",
    "equity": "3000",
    "revenue": "4000",
    "refunds": "4100",
    "fees": "6100",
    "shipping": "6200",
}


def ledger_entries(world: World) -> Rows:
    r = rng("finance__ledger_entries")
    log = ChangeLog("finance__ledger_entries")
    count = 0

    def post(
        journal: str,
        account: str,
        debit: int | None,
        credit: int | None,
        currency: str,
        when: datetime,
        reference: dict[str, object] | None,
    ) -> None:
        nonlocal count
        count += 1
        row = {
            "entry_id": f"JE{count:07d}",
            "journal_code": journal,
            "account_code": account,
            "debit_minor": debit,
            "credit_minor": credit,
            "currency_code": currency,
            "posted_at": in_zone(when, "Europe/London"),
            "reference": reference,
        }
        emit(log, r, [(when, row)], late_share=0.02)

    post(
        "opening",
        ACCOUNTS["cash"],
        25_000_000,
        None,
        "GBP",
        at(date(2026, 1, 1)),
        {"type": "Opening"},
    )
    post(
        "opening",
        ACCOUNTS["equity"],
        None,
        25_000_000,
        "GBP",
        at(date(2026, 1, 1)),
        {"type": "opening"},
    )
    for order in world.orders:
        paid = order.time_of("paid")
        if paid is None:
            continue
        reference = {"type": "order", "id": order.order_id}
        post(
            "sales",
            ACCOUNTS["receivables"],
            order.total_minor,
            None,
            order.currency,
            paid,
            reference,
        )
        post("sales", ACCOUNTS["revenue"], None, order.net_minor, order.currency, paid, reference)
        if order.tax_minor:
            post("SALES", ACCOUNTS["tax"], None, order.tax_minor, order.currency, paid, reference)
    for refund_id, _, order_id, amount, when, _ in refunds_plan(world.scale):
        currency = world.order_by_id[order_id].currency
        post(
            "refunds",
            ACCOUNTS["refunds"],
            amount,
            0,
            currency,
            when,
            {"type": "refund", "id": refund_id},
        )
        post(
            "refunds",
            ACCOUNTS["receivables"],
            0,
            amount,
            currency,
            when,
            {"type": "refund", "id": refund_id},
        )
    for payout in _payouts(world):
        reference = {"type": "payout", "id": payout["payout_id"]}
        when = at(payout["paid_out_on"], 6)  # type: ignore[arg-type]
        amount = int(payout["amount_minor"])  # type: ignore[call-overload]
        fee = int(payout["fee_minor"] or 0)  # type: ignore[call-overload]
        currency = str(payout["currency_code"])
        post("bank", ACCOUNTS["cash"], amount - fee, None, currency, when, reference)
        if fee:
            post("bank", ACCOUNTS["fees"], fee, None, currency, when, reference)
        post("bank", ACCOUNTS["receivables"], None, amount, currency, when, None)
    return log.rows()


def _payouts(world: World) -> list[dict[str, object]]:
    """Weekly payouts per provider and currency, built from captured payments and refunds."""

    r = rng("payouts_plan")
    orders = world.order_by_id
    groups: dict[tuple[str, str, date], list[dict[str, object]]] = defaultdict(list)
    for payment_id, order_id, method, amount, when, status in payments_plan(world.scale):
        if status not in ("captured", "settled"):
            continue
        payout_day = when.date() + timedelta(days=(7 - when.weekday()) % 7 + 2)
        if payout_day > date(2026, 3, 31):
            continue
        provider = provider_for(method)
        fee = max(1, amount * 14 // 1000)
        groups[(provider, orders[order_id].currency, payout_day)].append(
            {
                "payment_id": payment_id,
                "type": "charge" if chance(r, 0.95) else None,
                "gross_minor": amount,
                "fee_minor": fee,
            }
        )
    payment_method = {p[0]: p[2] for p in payments_plan(world.scale)}
    for _, payment_id, order_id, amount, when, _ in refunds_plan(world.scale):
        payout_day = when.date() + timedelta(days=(7 - when.weekday()) % 7 + 2)
        if payout_day > date(2026, 3, 31):
            continue
        key = (provider_for(payment_method[payment_id]), orders[order_id].currency, payout_day)
        if key in groups:
            groups[key].append({"payment_id": payment_id, "type": "Refund", "gross_minor": amount})
    payouts = []
    for index, ((provider, currency, day), lines) in enumerate(sorted(groups.items()), start=1):
        gross = sum(
            int(line["gross_minor"]) * (-1 if line["type"] == "Refund" else 1) for line in lines
        )  # type: ignore[call-overload]
        fee = sum(int(line.get("fee_minor", 0)) for line in lines)  # type: ignore[call-overload]
        if index % 17 == 0:
            lines = lines + [
                {"payment_id": "PAY9999999", "type": "charge", "gross_minor": 1234, "fee_minor": 17}
            ]
        payouts.append(
            {
                "payout_id": f"PO{index:05d}",
                "provider_code": provider.lower() if index % 5 == 0 else provider,
                "currency_code": currency,
                "amount_minor": gross,
                "fee_minor": fee if index % 9 else None,
                "paid_out_on": day,
                "lines": lines if index % 13 else None,
            }
        )
    return payouts


def payouts(world: World) -> Rows:
    r = rng("finance__payouts")
    log = ChangeLog("finance__payouts")
    for payout in _payouts(world):
        emit(log, r, [(at(payout["paid_out_on"], 5), payout)])  # type: ignore[arg-type]
    return log.rows()


CHARGEBACK_REASONS = ["10.4", "4837", "4863", "13.1", "4853", "4855", "12.6", "ab-01"]


def chargebacks(world: World) -> Rows:
    r = rng("finance__chargebacks")
    log = ChangeLog("finance__chargebacks")
    count = 0
    orders = world.order_by_id
    for payment_id, order_id, _, amount, when, status in payments_plan(world.scale):
        if status not in ("captured", "settled") or not chance(r, 0.012):
            continue
        count += 1
        opened = when + timedelta(days=r.randint(5, 40))
        if opened.date() > date(2026, 3, 31):
            continue
        order = orders[order_id]
        row = {
            "chargeback_id": f"CB{count:04d}",
            "payment_id": payment_id,
            "amount_minor": amount,
            "currency_code": order.currency.lower() if chance(r, 0.2) else order.currency,
            "reason_code": r.choice(CHARGEBACK_REASONS),
            "opened_at": in_zone(opened, "UTC"),
            "closed_at": None,
            "outcome": None,
            "evidence": r.choice(
                [
                    {"documents": [{"kind": "receipt"}, {"kind": "Tracking"}]},
                    {"documents": []},
                    {"documents": [{"kind": "receipt"}, {"kind": "RECEIPT"}, {"kind": None}]},
                    None,
                ]
            ),
        }
        versions = [(opened, row)]
        closed = opened + timedelta(days=r.randint(5, 30))
        if closed.date() <= date(2026, 3, 31) and chance(r, 0.7):
            versions.append(
                (
                    closed,
                    {
                        **row,
                        "closed_at": in_zone(closed, "UTC"),
                        "outcome": r.choice(["won", "Lost", "LOST", "accepted"]),
                    },
                )
            )
        emit(log, r, versions)
    return log.rows()


def settlement_reports(world: World) -> Rows:
    r = rng("finance__settlement_reports")
    log = EventLog("finance__settlement_reports")
    orders = world.order_by_id
    days: dict[tuple[str, date, str], list[dict[str, object]]] = defaultdict(list)
    for payment_id, order_id, method, amount, when, status in payments_plan(world.scale):
        if status not in ("captured", "settled") or when.date() < date(2026, 3, 1):
            continue
        settle_day = when.date() + timedelta(days=1)
        if settle_day > date(2026, 3, 31):
            continue
        line: dict[str, object]
        fee = max(1, amount * 14 // 1000)
        roll = r.random()
        if roll < 0.7:
            line = {
                "type": "charge",
                "reference": gateway_reference(payment_id),
                "gross_minor": amount,
                "fee_minor": fee,
            }
        elif roll < 0.9:
            line = {
                "record_type": "CHARGE",
                "description": f"Card payment pay_{gateway_reference(payment_id)}x",
                "gross": f"{amount / 100:.2f}",
                "fee_minor": fee,
            }
        else:
            line = {
                "type": "charge",
                "reference": "  ",
                "description": "Payment without a reference",
                "gross_minor": amount,
            }
        days[(provider_for(method), settle_day, orders[order_id].currency)].append(line)
    for refund_id, payment_id, order_id, amount, when, _ in refunds_plan(world.scale):
        settle_day = when.date() + timedelta(days=1)
        if when.date() < date(2026, 3, 1) or settle_day > date(2026, 3, 31):
            continue
        method = next(p[2] for p in payments_plan(world.scale) if p[0] == payment_id)
        days[(provider_for(method), settle_day, orders[order_id].currency)].append(
            {"type": "refund", "reference": refund_id, "gross_minor": -amount}
        )
    count = 0
    for (provider, day, currency), lines in sorted(days.items()):
        count += 1
        gross = sum(int(line.get("gross_minor", 0)) for line in lines)  # type: ignore[call-overload]
        fees = sum(int(line.get("fee_minor", 0)) for line in lines)  # type: ignore[call-overload]
        body = {
            "summary": {"currency": currency.lower(), "gross_minor": gross, "fees_minor": fees},
            "lines": lines,
        }
        loaded = at(day, 7, r.randint(0, 59))
        report_id = f"SR{count:05d}"
        log.add(
            {
                "report_id": report_id,
                "provider_code": provider,
                "report_date": day,
                "report_body": json.dumps(body),
            },
            loaded,
        )
        if chance(r, 0.05):
            # The provider re-sent the report later the same day with one more line.
            resent = {
                **body,
                "lines": lines + [{"type": "adjustment", "gross_minor": -5, "fee_minor": 0}],
            }
            log.add(
                {
                    "report_id": report_id,
                    "provider_code": provider,
                    "report_date": day,
                    "report_body": json.dumps(resent),
                },
                loaded + timedelta(hours=5),
            )
        if chance(r, 0.03):
            count += 1
            log.add(
                {
                    "report_id": f"SR{count:05d}",
                    "provider_code": provider.lower(),
                    "report_date": day,
                    "report_body": json.dumps({"summary": body["summary"], "lines": lines[:1]}),
                },
                loaded,
            )
    for broken, text in enumerate(
        ["<html><body>503 Service Unavailable</body></html>", "[]", "", '{"summary": '], start=1
    ):
        count += 1
        log.add(
            {
                "report_id": f"SR{count:05d}",
                "provider_code": "PROVIDER_A",
                "report_date": date(2026, 3, 10 + broken),
                "report_body": text or None,
            },
            at(date(2026, 3, 10 + broken), 7),
        )
    return log.rows()


TABLES = {
    "finance__invoices": invoices,
    "finance__ledger_entries": ledger_entries,
    "finance__payouts": payouts,
    "finance__tax_rates": tax_rates,
    "finance__chargebacks": chargebacks,
    "finance__settlement_reports": settlement_reports,
}

"""The shared synthetic world every DuckShop table is generated from.

Master data (storefronts, customers, products, warehouses) and the order history are built once per
scale, so every table agrees on the same customers, SKUs, orders and dates.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import date, datetime, timedelta
from functools import lru_cache

from python.nodes.loaders._core import (
    CLICKSTREAM_START,
    ORDERS_START,
    VALUATION_DATE,
    at,
    chance,
    random_instant,
    rng,
    weighted,
)


@dataclass(frozen=True)
class Storefront:
    code: str
    region: str
    timezone: str
    currency: str
    exponent: int
    weight: float


STOREFRONTS: dict[str, Storefront] = {
    s.code: s
    for s in (
        Storefront("GB", "EU", "Europe/London", "GBP", 2, 24),
        Storefront("DE", "EU", "Europe/Berlin", "EUR", 2, 15),
        Storefront("FR", "EU", "Europe/Paris", "EUR", 2, 8),
        Storefront("NL", "EU", "Europe/Amsterdam", "EUR", 2, 6),
        Storefront("US", "US", "America/New_York", "USD", 2, 20),
        Storefront("CA", "US", "America/Toronto", "USD", 2, 5),
        Storefront("JP", "APAC", "Asia/Tokyo", "JPY", 0, 9),
        Storefront("AU", "APAC", "Australia/Sydney", "AUD", 2, 8),
        Storefront("KW", "MENA", "Asia/Kuwait", "KWD", 3, 5),
    )
}

CURRENCY_EXPONENT = {"USD": 2, "EUR": 2, "GBP": 2, "JPY": 0, "AUD": 2, "KWD": 3}
# Rough units of each currency per US dollar, for pricing in local currency.
PER_USD = {"USD": 1.0, "EUR": 0.92, "GBP": 0.79, "JPY": 148.0, "AUD": 1.52, "KWD": 0.307}

FIRST_NAMES = [
    "Alex",
    "Sam",
    "Jordan",
    "Taylor",
    "Morgan",
    "Robin",
    "Casey",
    "Jamie",
    "Charlie",
    "Avery",
    "Riley",
    "Quinn",
    "Harper",
    "Rowan",
    "Sasha",
    "Kim",
    "Lee",
    "Noor",
    "Mina",
    "Yuki",
    "Hana",
    "Lena",
    "Jonas",
    "Emma",
    "Lucas",
    "Chloe",
    "Hugo",
    "Sofia",
    "Omar",
    "Layla",
]
LAST_NAMES = [
    "Smith",
    "Jones",
    "Brown",
    "Miller",
    "Davis",
    "Wilson",
    "Moore",
    "Clark",
    "Lewis",
    "Walker",
    "Muller",
    "Schmidt",
    "Fischer",
    "Martin",
    "Bernard",
    "Dubois",
    "de Vries",
    "van Dijk",
    "Bakker",
    "Tanaka",
    "Suzuki",
    "Sato",
    "Nguyen",
    "Kelly",
    "O Brien",
    "Murphy",
    "Rahman",
    "Haddad",
    "Ali",
]
CITIES = {
    "GB": [("London", "SW1A 1AA"), ("Leeds", "LS1 4AP"), ("Bristol", "BS1 5TR")],
    "DE": [("Berlin", "10115"), ("Hamburg", "20095"), ("Munich", "80331")],
    "FR": [("Paris", "75001"), ("Lyon", "69001")],
    "NL": [("Amsterdam", "1012 AB"), ("Utrecht", "3511 AA")],
    "US": [("New York", "10001"), ("Boston", "02108"), ("Denver", "80202")],
    "CA": [("Toronto", "M5H 2N2"), ("Ottawa", "K1P 1J1")],
    "JP": [("Tokyo", "100-0001"), ("Osaka", "5300001")],
    "AU": [("Sydney", "2000"), ("Melbourne", "3000")],
    "KW": [("Kuwait City", "13001"), ("Hawalli", "32001")],
}
GEO = {
    "GB": (51.5072, -0.1276),
    "DE": (52.52, 13.405),
    "FR": (48.8566, 2.3522),
    "NL": (52.3676, 4.9041),
    "US": (40.7128, -74.006),
    "CA": (43.6532, -79.3832),
    "JP": (35.6762, 139.6503),
    "AU": (-33.8688, 151.2093),
    "KW": (29.3759, 47.9774),
}


@dataclass(frozen=True)
class Warehouse:
    code: str
    name: str
    country: str
    timezone: str


WAREHOUSES = (
    Warehouse("POND", "The Pond", "GB", "Europe/London"),
    Warehouse("NEST", "The Nest", "US", "America/New_York"),
    Warehouse("MARSH", "The Marsh", "AU", "Australia/Sydney"),
)
WAREHOUSE_FOR_REGION = {"EU": "POND", "US": "NEST", "APAC": "MARSH", "MENA": "POND"}
CARRIERS = ("FLAPWING", "WINGBEAT", "POSTAL", "WEBFOOT")

# Product families are the leading letters of each SKU; duck species are the product lines.
SPECIES = [
    ("MAL", "Mallard"),
    ("TEAL", "Teal"),
    ("PIN", "Pintail"),
    ("WIG", "Wigeon"),
    ("EID", "Eider"),
    ("MER", "Merganser"),
    ("GOLD", "Goldeneye"),
    ("BUF", "Bufflehead"),
    ("SHOV", "Shoveler"),
    ("MAND", "Mandarin"),
    ("GAD", "Gadwall"),
    ("SCAUP", "Scaup"),
]
FAMILIES = [
    # family, title pattern, category path, base USD price, weight grams, brand
    ("DUCK", "{species} bath duck", "Ducks/Rubber-Ducks/bath-ducks", 9.0, 80, "MALLARD"),
    (
        "DUCK",
        "{species} collectible duck",
        "ducks//rubber-ducks/Collectible-Series/",
        24.0,
        120,
        "MALLARD",
    ),
    ("DECOY", "Carved {species} decoy", "decoys/carved-decoys", 89.0, 1400, "DABBLE"),
    ("DECOY", "Painted {species} decoy", "Decoys/painted-decoys", 59.0, 900, "DABBLE"),
    ("TOWEL", "{species} print towel", "textiles/towels", 19.0, 450, "FEATHERHOLM"),
    ("CUSHION", "{species} cushion cover", "textiles/cushions", 29.0, 300, "FEATHERHOLM"),
    ("MUG", "{species} mug", "home/mugs", 14.0, 350, "MALLARD"),
    ("POND", "{species} pond float", "pond-and-bath/pond-care", 34.0, 700, "GENERIC"),
]
COLOURS = ["Yellow", "green ", "Blue", "teal", "Natural", "Charcoal"]
SIZES = ["S", "M", "L", "x l", None]


@dataclass
class Customer:
    customer_id: str
    storefront: str
    first_name: str
    last_name: str
    email: str | None
    phone: str | None
    created_at: datetime
    addresses: list[str] = field(default_factory=list)
    default_address: str = ""
    anonymous_id: str = ""
    deleted: bool = False


@dataclass
class Variant:
    sku: str
    raw_sku: str
    product_id: str
    family: str
    species: str
    colour: str
    size: str | None
    weight_grams: int
    base_usd: float


@dataclass
class Product:
    product_id: str
    title: str
    category_path: str
    brand_code: str
    launched_on: date
    family: str
    species: str


@dataclass
class OrderLine:
    order_line_id: str
    line_no: int
    sku: str
    quantity: int
    unit_minor: int
    tax_minor: int
    discount_minor: int


@dataclass
class Order:
    order_id: str
    customer_id: str
    storefront: str
    currency: str
    exponent: int
    placed_at: datetime
    status: str
    status_times: list[tuple[str, datetime]]
    address_id: str
    channel: str | None
    utm: tuple[str, str, str] | None
    promo_code: str | None
    lines: list[OrderLine]
    gift_card_minor: int = 0
    gift_card_id: str | None = None

    @property
    def net_minor(self) -> int:
        return sum(line.quantity * line.unit_minor - line.discount_minor for line in self.lines)

    @property
    def tax_minor(self) -> int:
        return sum(line.tax_minor for line in self.lines)

    @property
    def total_minor(self) -> int:
        return self.net_minor + self.tax_minor

    def time_of(self, status: str) -> datetime | None:
        for name, when in self.status_times:
            if name == status:
                return when
        return None


@dataclass
class World:
    scale: int
    customers: list[Customer]
    products: list[Product]
    variants: list[Variant]
    orders: list[Order]
    promotions: dict[str, tuple[datetime, datetime, float]]

    @property
    def customer_by_id(self) -> dict[str, Customer]:
        return {c.customer_id: c for c in self.customers}

    @property
    def variant_by_sku(self) -> dict[str, Variant]:
        return {v.sku: v for v in self.variants}

    @property
    def order_by_id(self) -> dict[str, Order]:
        return {o.order_id: o for o in self.orders}


def local_price(base_usd: float, currency: str) -> int:
    """A list price in minor units: charm pricing for 2-decimal currencies, round for the rest."""

    exponent = CURRENCY_EXPONENT[currency]
    value = base_usd * PER_USD[currency]
    if exponent == 0:
        return int(round(value / 10) * 10)
    if exponent == 3:
        return int(round(value * 1000 / 5) * 5)
    return int(round(value)) * 100 - 1 if value >= 1 else int(round(value * 100))


def _products(scale: int) -> tuple[list[Product], list[Variant]]:
    r = rng("world", "products")
    products: list[Product] = []
    variants: list[Variant] = []
    count = 0
    numbers: dict[tuple[str, str], int] = {}
    for _ in range(scale):
        for family, title, path, base_usd, grams, brand in FAMILIES:
            for code, species in SPECIES:
                if not chance(r, 0.45):
                    continue
                count += 1
                product_id = f"P{count:04d}"
                launched = date(2024, 6, 1) + timedelta(days=r.randint(0, 600))
                brand_code = brand if not chance(r, 0.03) else "UNKNOWNBRAND"
                product_title = title.format(species=species)
                if chance(r, 0.05):
                    product_title = "  " + product_title.replace(" ", "  ", 1)
                products.append(
                    Product(product_id, product_title, path, brand_code, launched, family, species)
                )
                for _ in range(r.randint(1, 3)):
                    numbers[(family, code)] = numbers.get((family, code), 0) + 1
                    sku = f"{family}-{code}-{numbers[(family, code)]:02d}"
                    roll = r.random()
                    raw = (
                        sku
                        if roll < 0.8
                        else (sku.lower() if roll < 0.9 else sku.replace("-", "_", 1))
                    )
                    variants.append(
                        Variant(
                            sku=sku,
                            raw_sku=raw,
                            product_id=product_id,
                            family=family,
                            species=species,
                            colour=r.choice(COLOURS),
                            size=r.choice(SIZES),
                            weight_grams=grams + r.randint(-20, 60),
                            base_usd=round(base_usd * r.uniform(0.85, 1.3), 2),
                        )
                    )
    return products, variants


def _customers(scale: int) -> list[Customer]:
    r = rng("world", "customers")
    customers: list[Customer] = []
    count = 300 * scale
    codes = list(STOREFRONTS)
    weights = [STOREFRONTS[c].weight for c in codes]
    for i in range(1, count + 1):
        storefront = r.choices(codes, weights=weights, k=1)[0]
        first = r.choice(FIRST_NAMES)
        last = r.choice(LAST_NAMES)
        created = random_instant(r, date(2025, 1, 1), date(2026, 3, 20))
        domain = weighted(r, {"example.com": 85, "example.org": 8, "example.net": 7})
        local = f"{first}.{last}".lower().replace(" ", "")
        email: str | None = f"{local}{i}@{domain}"
        roll = r.random()
        if roll < 0.05:
            email = f"  {email.upper()} "
        elif roll < 0.09:
            email = email.replace("@", "+duckshop@")
        elif roll < 0.11:
            email = f"{local}{i} at {domain}"
        elif roll < 0.12:
            email = None
        phone: str | None = (
            f"+{r.randint(1, 99)} {r.randint(100, 999)} {r.randint(1000000, 9999999)}"
        )
        if chance(r, 0.15):
            phone = "0" + phone[1:].replace(" ", "-")
        if chance(r, 0.04):
            phone = "12-34"
        customers.append(
            Customer(
                customer_id=f"C{i:06d}",
                storefront=storefront,
                first_name=first,
                last_name=last,
                email=email,
                phone=phone,
                created_at=created,
                anonymous_id=f"anon-{r.getrandbits(48):012x}",
            )
        )
    # The order lookup report's customer: an established account with history.
    customers[0].created_at = at(date(2025, 2, 14), 9, 30)
    customers[0].storefront = "GB"
    # Duplicate accounts: the same person signed up twice with one phone number.
    for original_index in range(5, 5 + 4 * scale, 4):
        original = customers[original_index]
        duplicate = customers[original_index + 1]
        duplicate.first_name = original.first_name
        duplicate.last_name = original.last_name
        duplicate.phone = original.phone
        if original.email and duplicate.email:
            duplicate.email = original.email.strip().lower().replace("@", "+second@")
    # Accounts closed before ever ordering.
    for customer in customers[-3 * scale :]:
        customer.deleted = True
    for customer in customers:
        count = 2 if chance(r, 0.2) else 1
        customer.addresses = [f"A{customer.customer_id[1:]}-{n}" for n in range(1, count + 1)]
        customer.default_address = customer.addresses[-1]
    return customers


PROMOTIONS = {
    # code: (starts, ends, percent off)
    "SPLASH10": (at(date(2026, 1, 1)), at(date(2026, 3, 31), 23, 59), 10.0),
    "POND15": (at(date(2026, 2, 1)), at(date(2026, 2, 28), 23, 59), 15.0),
    "GOLDENEGG": (at(date(2026, 3, 1)), at(date(2026, 4, 30)), 20.0),
    "WELCOME5": (at(date(2025, 6, 1)), at(date(2026, 12, 31)), 5.0),
}
CAMPAIGNS = [
    # campaign id, name, channel, starts, ends, budget in GBP pence, utm source and medium
    (
        "WINTER-WADDLE",
        "Winter waddle sale",
        "paid_search",
        date(2026, 1, 5),
        date(2026, 1, 31),
        300000,
        ("search_ads", "cpc"),
    ),
    (
        "POND-WEEK",
        "Pond week",
        "paid_social",
        date(2026, 2, 1),
        date(2026, 2, 28),
        250000,
        ("social_ads", "paid_social"),
    ),
    (
        "POND-WEEK-EMAIL",
        "Pond week newsletter",
        "email",
        date(2026, 2, 1),
        date(2026, 2, 28),
        20000,
        ("newsletter", "email"),
    ),
    (
        "SPRING-SPLASH",
        "Spring splash",
        "paid_search",
        date(2026, 3, 1),
        date(2026, 4, 15),
        500000,
        ("search_ads", "cpc"),
    ),
    (
        "GOLDEN-EGG",
        "Golden egg hunt",
        "influencer",
        date(2026, 3, 1),
        date(2026, 4, 30),
        150000,
        ("creator", "influencer"),
    ),
    (
        "ALWAYS-ON",
        "Always-on affiliates",
        "affiliate",
        date(2026, 1, 1),
        None,
        400000,
        ("partner_site", "affiliate"),
    ),
]


def utm_for(when: datetime) -> list[tuple[str, str, str]]:
    """UTM tags of the campaigns running on a day; the campaign tag is the lower-cased id."""

    day = when.date()
    tags = [
        (source, medium, campaign_id.lower())
        for campaign_id, _, _, starts, ends, _, (source, medium) in CAMPAIGNS
        if starts <= day and (ends is None or day <= ends)
    ]
    return tags + [("", "organic", ""), ("", "referral", "")]


STATUS_FLOW = ["pending", "paid", "packed", "shipped", "delivered"]


def _orders(customers: list[Customer], variants: list[Variant], scale: int) -> list[Order]:
    r = rng("world", "orders")
    live = [c for c in customers if not c.deleted]
    orders: list[Order] = []
    count = 1200 * scale
    end = VALUATION_DATE
    # A few orders pinned to daylight-saving changes and to the edges of the reporting window.
    pinned = [
        datetime(2026, 3, 8, 6, 55),  # US clocks go forward at 07:00 UTC
        datetime(2026, 3, 8, 7, 5),
        datetime(2026, 3, 29, 0, 55),  # EU clocks go forward at 01:00 UTC
        datetime(2026, 3, 29, 1, 10),
        datetime(2026, 1, 1, 0, 0, 1),
        datetime(2026, 3, 31, 23, 59, 59),
        datetime(2026, 3, 31, 14, 30),  # after the Tokyo and Sydney business day ended
    ]
    for i in range(1, count + 1):
        customer = live[0] if i % 97 == 1 else r.choice(live)
        sf = STOREFRONTS[customer.storefront]
        if i <= len(pinned):
            placed = pinned[i - 1]
        else:
            placed = random_instant(r, ORDERS_START, end)
        if placed < customer.created_at:
            placed = customer.created_at + timedelta(minutes=r.randint(5, 600))
            if placed.date() > end:
                placed = at(end, 12)
        # Status progression; recent orders have not finished their journey yet.
        age_days = (at(end, 23, 59) - placed).total_seconds() / 86400
        times: list[tuple[str, datetime]] = [("pending", placed)]
        status = "pending"
        roll = r.random()
        if roll < 0.03:
            status = "pending"
        elif roll < 0.08:
            status = "cancelled"
            times.append(("cancelled", placed + timedelta(hours=r.randint(1, 30))))
        else:
            when = placed + timedelta(minutes=r.randint(1, 20))
            times.append(("paid", when))
            status = "paid"
            for step, hours in (
                ("packed", (4, 30)),
                ("shipped", (2, 40)),
                ("delivered", (20, 120)),
            ):
                when = when + timedelta(hours=r.randint(*hours), minutes=r.randint(0, 59))
                if (when - placed).total_seconds() / 86400 > age_days:
                    break
                times.append((step, when))
                status = step
            if status == "delivered" and chance(r, 0.06):
                when = when + timedelta(days=r.randint(3, 12))
                if (when - placed).total_seconds() / 86400 <= age_days:
                    times.append(("returned", when))
                    status = "returned"
        lines: list[OrderLine] = []
        promo = None
        for code, (starts, ends, _) in PROMOTIONS.items():
            if starts <= placed <= ends and chance(r, 0.08):
                promo = code
                break
        pct = PROMOTIONS[promo][2] if promo else 0.0
        line_count = weighted(r, {1: 45, 2: 30, 3: 15, 4: 6, 6: 4})
        used: set[str] = set()
        for n in range(1, int(line_count) + 1):  # type: ignore[call-overload]
            variant = r.choice(variants)
            if variant.sku in used:
                continue
            used.add(variant.sku)
            quantity = weighted(r, {1: 70, 2: 18, 3: 7, 6: 4, 12: 1})
            unit = local_price(variant.base_usd, sf.currency)
            if chance(r, 0.1):
                unit = int(unit * 0.8)
            gross = int(quantity) * unit  # type: ignore[call-overload]
            discount = int(round(gross * pct / 100)) if promo else 0
            tax_rate = {"EU": 0.2, "US": 0.0, "APAC": 0.1, "MENA": 0.0}[sf.region]
            tax = int(round((gross - discount) * tax_rate))
            lines.append(
                OrderLine(
                    order_line_id=f"OL{i:06d}-{n}",
                    line_no=n,
                    sku=variant.sku,
                    quantity=int(quantity),  # type: ignore[call-overload]
                    unit_minor=unit,
                    tax_minor=tax,
                    discount_minor=discount,
                )
            )
        utm = r.choice(utm_for(placed)) if chance(r, 0.45) else None
        channel = weighted(r, {"web": 62, "app": 22, "store": 5, "phone": 3, None: 8})
        orders.append(
            Order(
                order_id=f"DS-O-{i:06d}",
                customer_id=customer.customer_id,
                storefront=customer.storefront,
                currency=sf.currency,
                exponent=sf.exponent,
                placed_at=placed,
                status=status,
                status_times=times,
                address_id=customer.default_address,
                channel=channel,  # type: ignore[arg-type]
                utm=utm,
                promo_code=promo,
                lines=lines,
            )
        )
    orders.sort(key=lambda o: (o.placed_at, o.order_id))
    return orders


@lru_cache(maxsize=4)
def build_world(scale: int = 1) -> World:
    """The deterministic world for one volume scale. Scale 1 is the small default."""

    if scale < 1:
        raise ValueError("scale must be a positive integer")
    products, variants = _products(scale)
    customers = _customers(scale)
    orders = _orders(customers, variants, scale)
    return World(
        scale=scale,
        customers=customers,
        products=products,
        variants=variants,
        orders=orders,
        promotions=PROMOTIONS,
    )


def clickstream_days() -> list[date]:
    return [
        CLICKSTREAM_START + timedelta(days=i)
        for i in range((VALUATION_DATE - CLICKSTREAM_START).days + 1)
    ]

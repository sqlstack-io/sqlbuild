"""Catalogue change logs: brands, categories, products, variants, prices, media and suppliers."""

from __future__ import annotations

from datetime import date, datetime, timedelta

from python.nodes.loaders._core import ChangeLog, at, chance, emit, in_zone, rng
from python.nodes.loaders._world import CURRENCY_EXPONENT, World, local_price

Rows = list[dict[str, object]]

BRANDS = [
    # code as sent, name, country, private label, metadata
    (
        "MALLARD",
        "Mallard & Co.",
        "gb",
        True,
        {"founded": "est. 1998", "sustainability": {"rating": "a"}},
    ),
    (
        "featherholm",
        " Featherholm Textiles ",
        "PT",
        False,
        {"founded": 2004, "sustainability": {"rating": None}},
    ),
    (
        "DABBLE",
        "Dabbleworth Decoys",
        "us",
        False,
        {"founded": "Founded in the year 1987", "awards": []},
    ),
    ("GENERIC", "Unbranded pond goods", "CN", None, {"sustainability": {"rating": "  "}}),
]

CATEGORIES = [
    # id, parent, slug as sent, display name, sort order, translations
    ("C01", None, "Ducks", "Ducks", 1, {"en": "Ducks", "de": "Enten"}),
    ("C02", "C01", "Rubber Ducks", "Rubber ducks", 1, {"en": "Rubber ducks", "de": "Badeenten"}),
    ("C03", "C02", "bath_ducks", "Bath ducks", 1, None),
    ("C04", "C02", "collectible-series", "Collectible series", 2, {"en": "Collectible series"}),
    ("C05", None, "decoys", "Decoys", 2, {"en": "Decoys", "de": "Lockenten", "fr": "Appelants"}),
    ("C06", "C05", "carved decoys", "Carved decoys", 1, None),
    ("C07", "C05", "painted-decoys", "Painted decoys", None, None),
    ("C08", "C08", "textiles", "Textiles", 3, {"en": "Textiles"}),
    ("C09", "C08", "towels", "Towels", 1, None),
    ("C10", "C08", "cushions", "Cushions", 2, None),
    ("C11", None, "home", "Home", 4, None),
    ("C12", "C11", "mugs", "Mugs", 1, {"de": "Tassen"}),
    ("C13", None, "pond-and-bath", "Pond & bath", 5, None),
    ("C14", "C13", "pond-care", "Pond care", 1, None),
    ("C15", "C99", "orphaned", "Orphaned category", 9, None),
]

SPECS = {
    "DUCK": lambda r: {
        "material": r.choice(["vinyl", "natural rubber", "Vinyl"]),
        "Height (cm)": f"{r.randint(6, 14)} cm",
        "colour_family": r.choice(["Yellow", "white", "multi"]),
        "dishwasher_safe": r.choice(["yes", "no", True, False]),
    },
    "DECOY": lambda r: {
        "material": r.choice(["cedar", "cork", "basswood"]),
        "width": f"{r.randint(250, 420)} mm",
        "height_cm": round(r.uniform(9.0, 21.0), 7),
        "carver": None,
    },
    "TOWEL": lambda r: {
        "material": "cotton",
        "width_cm": r.randint(50, 100),
        "height": f"{r.choice([0.9, 1.4, 1.5])} m",
        "gsm": r.choice([450, 600, "600"]),
    },
    "CUSHION": lambda r: {"material": "linen", "width": "45cm", "height": "45 cm", "filling": None},
    "MUG": lambda r: {
        "material": "stoneware",
        "capacity": r.choice([300, 350.0, 400.125]),
        "dishwasher_safe": "TRUE",
        "colour_family": "Natural",
    },
    "POND": lambda r: {"material": "recycled plastic", "capacity_ml": 2500, "uv_stable": True},
}

SUPPLIERS = [
    # code, name, country, payment terms, contacts
    (
        "VINYLWORKS",
        "Vinyl Moulding Works",
        "CN",
        {"net_days": 60},
        [
            {"email": "Orders@Vinyl-Moulding.example.com", "is_primary": True},
            {"email": "accounts@vinyl-moulding.example.com"},
        ],
    ),
    (
        "COASTMILL",
        "Coastal Textile Mill",
        "PT",
        {"label": "Net 45, 2% 10"},
        [
            {"email": "sales@coastal-mill.example.com"},
        ],
    ),
    (
        "CARVECOOP",
        "Hand Carving Cooperative",
        "US",
        {"label": "NET30", "early_discount_pct": 1.5},
        [],
    ),
    (
        "CERAMICS",
        "Ceramics Workshop",
        "GB",
        {"net_days": 30},
        [
            {"email": "not an email"},
            {"email": "hello@ceramics.example.com", "is_primary": True},
        ],
    ),
    ("pondsupply", "Pond Supply Wholesale", "NL", {}, None),
]
SUPPLIER_FOR_FAMILY = {
    "DUCK": ["VINYLWORKS"],
    "DECOY": ["CARVECOOP"],
    "TOWEL": ["COASTMILL"],
    "CUSHION": ["COASTMILL"],
    "MUG": ["CERAMICS"],
    "POND": ["pondsupply", "VINYLWORKS"],
}
SUPPLIER_CURRENCY = {
    "VINYLWORKS": "USD",
    "COASTMILL": "EUR",
    "CARVECOOP": "USD",
    "CERAMICS": "GBP",
    "pondsupply": "EUR",
}
CATALOGUE_TZ = "Europe/London"


def _at(day: date, hour: int = 9) -> datetime:
    return at(day, hour)


def brands(world: World) -> Rows:
    r = rng("catalogue__brands")
    log = ChangeLog("catalogue__brands")
    for code, name, country, private, metadata in BRANDS:
        row = {
            "brand_code": code,
            "brand_name": name,
            "country_code": country,
            "is_private_label": private,
            "metadata": metadata,
        }
        versions = [(_at(date(2025, 11, 3)), row)]
        if code == "DABBLE":
            versions.append(
                (
                    _at(date(2026, 2, 10)),
                    {**row, "metadata": {**metadata, "awards": ["Best decoy 2025"]}},
                )
            )
        emit(log, r, versions)
    retired = {
        "brand_code": "OLDPOND",
        "brand_name": "Retired pond brand",
        "country_code": "GB",
        "is_private_label": False,
        "metadata": None,
    }
    emit(log, r, [(_at(date(2025, 11, 3)), retired)], delete_at=_at(date(2026, 1, 15)))
    return log.rows()


def categories(world: World) -> Rows:
    r = rng("catalogue__categories")
    log = ChangeLog("catalogue__categories")
    for category_id, parent, slug, name, order, translations in CATEGORIES:
        row = {
            "category_id": category_id,
            "parent_category_id": parent,
            "slug": slug,
            "display_name": name,
            "sort_order": order,
            "translations": translations,
        }
        versions = [(_at(date(2025, 10, 1)), row)]
        if category_id == "C04":
            versions.append((_at(date(2026, 3, 2)), {**row, "sort_order": 0}))
        emit(log, r, versions, duplicate_share=0.05)
    return log.rows()


def products(world: World) -> Rows:
    r = rng("catalogue__products")
    log = ChangeLog("catalogue__products")
    for product in world.products:
        row = {
            "product_id": product.product_id,
            "title": product.title,
            "category_path": product.category_path,
            "brand_code": product.brand_code,
            "launched_on": product.launched_on,
            "is_active": None if chance(r, 0.1) else True,
            "specs": SPECS[product.family](r),
        }
        versions = [(_at(product.launched_on - timedelta(days=14)), row)]
        if chance(r, 0.15):
            versions.append(
                (
                    _at(date(2026, 2, 1) + timedelta(days=r.randint(0, 50)), 11),
                    {**row, "is_active": False},
                )
            )
        emit(log, r, versions)
    draft = {
        "product_id": "P9999",
        "title": "Draft listing created by mistake",
        "category_path": "ducks",
        "brand_code": "MALLARD",
        "launched_on": None,
        "is_active": False,
        "specs": {},
    }
    emit(log, r, [(_at(date(2026, 1, 20)), draft)], delete_at=_at(date(2026, 1, 21)))
    return log.rows()


def product_variants(world: World) -> Rows:
    r = rng("catalogue__product_variants")
    log = ChangeLog("catalogue__product_variants")
    seen_barcodes: list[str] = []
    for index, variant in enumerate(world.variants):
        barcode = f"50{index:010d}"
        barcode = barcode + str(_ean_check(barcode))
        codes: list[object] = [barcode]
        roll = r.random()
        if roll < 0.15:
            codes.append(f"0{index:010d}5")  # a 12-digit UPC-A
        elif roll < 0.2:
            codes.append(f"9638{index:04d}")  # an 8-digit EAN-8
        elif roll < 0.25 and seen_barcodes:
            codes.append(seen_barcodes[-1])  # a barcode shared by two variants
        elif roll < 0.28:
            codes.append("N/A")
        seen_barcodes.append(barcode)
        attributes: dict[str, object] = {"pack_size": r.choice([1, 1, 1, "6", 12])}
        if variant.family in ("MUG", "DECOY"):
            attributes["handling"] = {"fragile": True}
        row = {
            "sku": variant.raw_sku,
            "product_id": variant.product_id,
            "colour": variant.colour,
            "size_label": variant.size,
            "weight_grams": variant.weight_grams,
            "barcodes": codes,
            "attributes": attributes,
        }
        versions = [(_at(date(2025, 11, 1)), row)]
        if chance(r, 0.1):
            versions.append(
                (_at(date(2026, 2, 20)), {**row, "weight_grams": variant.weight_grams + 15})
            )
        emit(log, r, versions)
    return log.rows()


def _ean_check(digits: str) -> int:
    total = sum(int(d) * (3 if i % 2 else 1) for i, d in enumerate(digits))
    return (10 - total % 10) % 10


def price_lists(world: World) -> Rows:
    r = rng("catalogue__price_lists")
    log = ChangeLog("catalogue__price_lists")
    count = 0
    for variant in world.variants:
        for currency in ("GBP", "EUR", "USD", "JPY", "AUD", "KWD"):
            zone = {
                "GBP": "Europe/London",
                "EUR": "Europe/Berlin",
                "USD": "America/New_York",
                "JPY": "Asia/Tokyo",
                "AUD": "Australia/Sydney",
                "KWD": "Asia/Kuwait",
            }[currency]
            base = local_price(variant.base_usd, currency)
            count += 1
            opened = at(date(2025, 12, 1))
            first: dict[str, object] = {
                "price_list_id": f"PL{count:06d}",
                "sku": variant.raw_sku if chance(r, 0.8) else variant.sku.lower(),
                "currency_code": currency if chance(r, 0.9) else currency.lower(),
                "list_amount_minor": base,
                "valid_from": in_zone(opened, zone),
                "valid_to": None,
                "conditions": None,
            }
            markdown = chance(r, 0.2)
            if markdown:
                change_at = at(date(2026, 2, 1) + timedelta(days=r.randint(0, 40)))
                first["valid_to"] = in_zone(change_at, zone)
                emit(log, r, [(opened, first)], duplicate_share=0.01)
                count += 1
                new_amount = (
                    int(base * 0.75) if CURRENCY_EXPONENT[currency] else int(base * 0.75 / 10) * 10
                )
                second = {
                    **first,
                    "price_list_id": f"PL{count:06d}",
                    "list_amount_minor": new_amount,
                    "valid_from": in_zone(change_at, zone),
                    "valid_to": in_zone(datetime(9999, 12, 31), "UTC") if chance(r, 0.5) else None,
                }
                emit(log, r, [(change_at, second)], duplicate_share=0.01)
            else:
                versions = [(opened, first)]
                if chance(r, 0.05):
                    versions.append(
                        (
                            at(date(2026, 1, 10)),
                            {**first, "conditions": {"min_qty": 1, "note": None}},
                        )
                    )
                emit(log, r, versions, duplicate_share=0.01)
            if currency == "GBP" and chance(r, 0.08):
                count += 1
                app_price = {
                    **first,
                    "price_list_id": f"PL{count:06d}",
                    "list_amount_minor": int(base * 0.9),
                    "valid_from": in_zone(at(date(2026, 1, 5)), zone),
                    "valid_to": None,
                    "conditions": {"channels": ["app"], "min_qty": "2"},
                }
                emit(log, r, [(at(date(2026, 1, 5)), app_price)])
    return log.rows()


def product_media(world: World) -> Rows:
    r = rng("catalogue__product_media")
    log = ChangeLog("catalogue__product_media")
    count = 0
    for product in world.products:
        for position in range(1, r.randint(1, 4) + 1):
            count += 1
            kind = "image" if position < 3 else r.choice(["video", "Image", "model3d"])
            ext = {
                "image": r.choice(["jpg", "PNG", "webp"]),
                "Image": "jpeg",
                "video": "mp4",
                "model3d": "glb",
            }[kind]
            scheme = "http" if chance(r, 0.2) else "https"
            url = f"{scheme}://media.example.com/{product.product_id.lower()}/{position}.{ext}"
            if chance(r, 0.1):
                url += "?v=2"
            dimensions: dict[str, object] | None = {
                "width": r.choice([800, 1200, 1600]),
                "height": r.choice([800, 1200, 0]),
            }
            if kind in ("video", "model3d"):
                dimensions = None
            row = {
                "media_id": f"M{count:05d}",
                "product_id": product.product_id,
                "url": url,
                "media_type": kind,
                "position": position,
                "dimensions": dimensions,
            }
            emit(log, r, [(_at(product.launched_on), row)])
    return log.rows()


def bundles(world: World) -> Rows:
    r = rng("catalogue__bundles")
    log = ChangeLog("catalogue__bundles")
    for code, title, active_from, active_to, pricing in _bundle_definitions():
        row = {
            "bundle_sku": code,
            "title": title,
            "active_from": active_from,
            "active_to": active_to,
            "pricing": pricing,
        }
        emit(log, r, [(_at(active_from - timedelta(days=7)), row)])
    return log.rows()


def _bundle_definitions():
    return [
        (
            "BUNDLE-PADDLING-SIX",
            "Paddling Six mini-duck set",
            date(2025, 12, 1),
            None,
            {"mode": "percent_off", "percent_off": 12.5},
        ),
        (
            "bundle-bath-time",
            "Bath time gift set",
            date(2026, 1, 1),
            date(2026, 3, 31),
            {"mode": "FIXED", "amount_minor": 2999, "currency": "gbp"},
        ),
        ("BUNDLE-DECOY-PAIR", "Decoy pair", date(2026, 2, 1), None, {"mode": "cheapest_free"}),
        ("BUNDLE-POND-STARTER", "Pond starter kit", date(2026, 3, 1), None, None),
    ]


def bundle_components(world: World) -> Rows:
    r = rng("catalogue__bundle_components")
    log = ChangeLog("catalogue__bundle_components")
    by_family: dict[str, list[str]] = {}
    for variant in world.variants:
        by_family.setdefault(variant.family, []).append(variant.raw_sku)
    plan = {
        "BUNDLE-PADDLING-SIX": [("DUCK", 6), ("DUCK", 1)],
        "bundle-bath-time": [("DUCK", 2), ("TOWEL", 1), ("MUG", 1)],
        "BUNDLE-DECOY-PAIR": [("DECOY", 1), ("DECOY", 1)],
        "BUNDLE-POND-STARTER": [("POND", 1), ("DUCK", None)],
    }
    count = 0
    for bundle, parts in plan.items():
        for index, (family, quantity) in enumerate(parts):
            count += 1
            options: dict[str, object] | None = None
            if index == len(parts) - 1 and len(parts) > 2:
                options = {"optional": True, "substitutes": by_family[family][:2]}
            row = {
                "component_id": f"BC{count:03d}",
                "bundle_sku": bundle,
                "component_sku": r.choice(by_family[family]),
                "quantity": quantity,
                "options": options,
            }
            emit(log, r, [(_at(date(2025, 11, 25)), row)])
    return log.rows()


def suppliers(world: World) -> Rows:
    r = rng("catalogue__suppliers")
    log = ChangeLog("catalogue__suppliers")
    for code, name, country, terms, contacts in SUPPLIERS:
        row = {
            "supplier_code": code,
            "supplier_name": name,
            "country_code": country,
            "payment_terms": terms,
            "contacts": contacts,
        }
        versions = [(_at(date(2025, 9, 1)), row)]
        if code == "COASTMILL":
            versions.append((_at(date(2026, 1, 12)), {**row, "payment_terms": {"label": "net 60"}}))
        emit(log, r, versions)
    return log.rows()


def supplier_costs(world: World) -> Rows:
    r = rng("catalogue__supplier_costs")
    log = ChangeLog("catalogue__supplier_costs")
    count = 0
    for variant in world.variants:
        for supplier in SUPPLIER_FOR_FAMILY[variant.family]:
            currency = SUPPLIER_CURRENCY[supplier]
            exponent = CURRENCY_EXPONENT[currency]
            unit = int(round(variant.base_usd * r.uniform(0.25, 0.45) * 10**exponent))
            effective = [date(2026, 1, 1)]
            if chance(r, 0.25):
                effective.append(date(2026, 3, 1) + timedelta(days=r.randint(0, 29)))
            for start in effective:
                count += 1
                breaks: list[dict[str, object]] | None = None
                if chance(r, 0.3):
                    breaks = [
                        {"min_qty": 100, "unit_cost_minor": int(unit * 0.95)},
                        {"min_qty": 500, "unit_cost_minor": int(unit * 0.9)},
                    ]
                row = {
                    "cost_id": f"SC{count:05d}",
                    "supplier_code": supplier,
                    "sku": variant.raw_sku,
                    "currency_code": currency,
                    "unit_cost_minor": unit if start.month == 1 else int(unit * 1.04),
                    "effective_from": start,
                    "volume_breaks": breaks,
                }
                emit(log, r, [(_at(start - timedelta(days=10)), row)])
    return log.rows()


TABLES = {
    "catalogue__brands": brands,
    "catalogue__categories": categories,
    "catalogue__products": products,
    "catalogue__product_variants": product_variants,
    "catalogue__price_lists": price_lists,
    "catalogue__product_media": product_media,
    "catalogue__bundles": bundles,
    "catalogue__bundle_components": bundle_components,
    "catalogue__suppliers": suppliers,
    "catalogue__supplier_costs": supplier_costs,
}

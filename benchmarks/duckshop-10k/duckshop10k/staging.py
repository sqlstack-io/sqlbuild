"""Sources and the staging layer: base_ change-log dedupe, typed stg_ models and their fan-outs."""

from __future__ import annotations

from dataclasses import dataclass

from duckshop10k.core import Column, Model, Project, cte, import_cte, select_list
from duckshop10k.specs import (
    ARCHETYPES,
    EVENT_TYPES,
    Archetype,
    Child,
    Field,
    json_path,
    lives_in_payload,
    payload_column,
    physical_name,
    physical_type,
    staging_name,
    staging_type,
)
from duckshop10k.world import Instance

RAW_DATABASE = "DUCKSHOP_RAW"


@dataclass(frozen=True)
class StagedTable:
    """What later layers need to know about one staged source table."""

    instance: Instance
    archetype: Archetype
    stg: str
    children: dict[str, str]
    history: str | None


def model_type(source_type: str) -> str:
    return source_type.replace(",", ", ")


def with_query(parts: list[str]) -> str:
    """WITH list in house style; CTE-list macro calls are spliced in without a comma."""
    out = "WITH "
    for index, part in enumerate(parts):
        out += part
        if index == len(parts) - 1:
            break
        out += "\n\n" if part.startswith("@sql.") else ",\n\n"
    return out + "\n\nSELECT *\nFROM final\n"


# --- Field transforms -------------------------------------------------------------------------


def json_transform(role: str, value: str, platform: str) -> str:
    text = f"{value}::VARCHAR"
    return {
        "key": f"REGEXP_REPLACE(@sql.json_text({value}), '[^A-Za-z0-9_:.-]', '')::VARCHAR",
        "id": f"@sql.json_text({value})",
        "code": f"UPPER(@sql.json_text({value}))::VARCHAR",
        "email": f"@sql.normalise_email({text})",
        "phone": f"@sql.normalise_phone({text})",
        "sku": f"@sql.normalise_sku({text})",
        "text": f"NULLIF(TRIM(REGEXP_REPLACE({text}, '\\\\s+', ' ')), '')::VARCHAR",
        "name": f"INITCAP(TRIM({text}))::VARCHAR",
        "country": f"UPPER(TRIM({text}))::VARCHAR",
        "currency": f"UPPER(TRIM({text}))::VARCHAR",
        "postcode": f"UPPER(REGEXP_REPLACE({text}, '[^A-Za-z0-9]', ''))::VARCHAR",
        "url": f"('/' || LTRIM(PARSE_URL({text}, 1)['path']::VARCHAR, '/'))::VARCHAR",
        "amount": f"TRY_TO_NUMBER({text})::NUMBER(38, 0)",
        "qty": f"TRY_TO_NUMBER({text})::INT",
        "int": f"TRY_TO_NUMBER({text})::INT",
        "rate": f"TRY_TO_DECIMAL({text}, 18, 6)::NUMBER(18, 6)",
        "ts": (
            f"@sql.epoch_ms_to_utc(TRY_TO_NUMBER({text})::NUMBER(38, 0))"
            if platform == "flock"
            else f"TRY_TO_TIMESTAMP_TZ({text})"
        ),
        "date": f"TRY_TO_DATE({text})",
        "bool": f"COALESCE({value}::BOOLEAN, FALSE)",
        "status": f"LOWER(TRIM({text}))::VARCHAR",
        "array": f"COALESCE({value}, ARRAY_CONSTRUCT())::ARRAY",
        "object": f"COALESCE({value}, OBJECT_CONSTRUCT())::OBJECT",
    }[role]


def column_transform(role: str, value: str, platform: str) -> str:
    classic = platform == "classic"
    return {
        "key": f"REGEXP_REPLACE(TRIM({value}), '[^A-Za-z0-9_:.-]', '')::VARCHAR",
        "id": f"TRIM({value})::VARCHAR",
        "code": f"UPPER(TRIM({value}))::VARCHAR",
        "email": f"@sql.normalise_email({value})",
        "phone": f"@sql.normalise_phone({value})",
        "sku": f"@sql.normalise_sku({value})",
        "text": f"NULLIF(TRIM(REGEXP_REPLACE({value}, '\\\\s+', ' ')), '')::VARCHAR",
        "name": f"INITCAP(TRIM({value}))::VARCHAR",
        "country": f"UPPER(TRIM({value}))::VARCHAR",
        "currency": f"UPPER(TRIM({value}))::VARCHAR",
        "postcode": f"UPPER(REGEXP_REPLACE({value}, '[^A-Za-z0-9]', ''))::VARCHAR",
        "url": f"('/' || LTRIM(PARSE_URL({value}, 1)['path']::VARCHAR, '/'))::VARCHAR",
        "amount": (f"ROUND({value}, 2)::NUMBER(18, 2)" if classic else f"{value}::NUMBER(38, 0)"),
        "qty": f"{value}::INT",
        "int": f"{value}::INT",
        "rate": f"{value}::NUMBER(18, 6)",
        "ts": f"{value}::TIMESTAMP_NTZ" if classic else f"{value}::TIMESTAMP_TZ",
        "date": f"{value}::DATE",
        "bool": (
            f"COALESCE(UPPER({value}) = 'Y', FALSE)::BOOLEAN"
            if classic
            else f"COALESCE({value}, FALSE)::BOOLEAN"
        ),
        "status": f"{value}::INT" if classic else f"LOWER(TRIM({value}))::VARCHAR",
    }[role]


def key_field(archetype: Archetype) -> Field:
    return Field(archetype.key, "key")


# --- Sources ----------------------------------------------------------------------------------


def physical_columns(archetype: Archetype, platform: str) -> list[tuple[str, str]]:
    """(name, type) of every raw column, excluding the change-log metadata."""
    key = key_field(archetype)
    columns = [(physical_name(key, platform), "VARCHAR")]
    if platform == "flock":
        return [*columns, ("doc", "VARIANT")]
    for item in archetype.fields:
        if not lives_in_payload(item, platform):
            columns.append((physical_name(item, platform), physical_type(item, platform)))
    if any(item.in_payload for item in archetype.fields):
        payload = "extra_json" if platform == "classic" else "attributes"
        columns.append((payload, "VARCHAR" if platform == "classic" else "VARIANT"))
    return columns


def source_yaml(instance: Instance, archetype: Archetype) -> str:
    name = f"{instance.code}__{archetype.table}"
    lines = [
        f"  - name: {name}",
        f"    description: {archetype.description.capitalize()} from {instance.label}"
        + (", append-only" if archetype.append else ", change log"),
        f"    database: {RAW_DATABASE}",
        f"    schema: {instance.code.upper()}",
        f"    table: {archetype.table.upper()}",
        "    contract: enforced",
        "    columns:",
    ]
    for index, (column, column_type) in enumerate(physical_columns(archetype, instance.platform)):
        lines.append(f"      - name: {column}")
        lines.append(f"        type: {column_type}")
        if index == 0:
            lines.extend(["        nullable: false", "        audits:", "          - not_null"])
    if not archetype.append:
        lines.extend(
            [
                "      - name: _op",
                "        type: VARCHAR",
                "        nullable: false",
                "        audits:",
                "          - accepted_values:",
                '              values: ["I", "U", "D"]',
            ]
        )
    lines.extend(
        [
            "      - name: _seq",
            "        type: NUMBER(38,0)",
            "        nullable: false",
            "        audits:",
            "          - not_null",
            "          - unique",
            "      - name: _loaded_at",
            "        type: TIMESTAMP_NTZ",
            "        nullable: false",
            "      - name: _meta",
            "        type: VARIANT",
        ]
    )
    return "\n".join(lines)


# --- base_ models -----------------------------------------------------------------------------


def base_model(project: Project, instance: Instance, archetype: Archetype, folder: str) -> Model:
    source = f"{instance.code}__{archetype.table}"
    platform = instance.platform
    raw = physical_columns(archetype, platform)
    key = raw[0][0]
    ctes = [import_cte("src", f'__source("{source}")')]
    if archetype.append:
        ctes.append(
            cte(
                "deduplicated",
                "SELECT *\nFROM src AS s\n"
                f"QUALIFY ROW_NUMBER() OVER (PARTITION BY s.{key} ORDER BY s._seq DESC) = 1",
            )
        )
        relation = "deduplicated"
    else:
        ctes.append(f"@sql.cdc_latest(src, src.key := ({key}))")
        relation = "cdc_latest"
    items: list[str] = []
    columns: list[Column] = []
    for index, (name, column_type) in enumerate(raw):
        if name == "extra_json":
            items.append("TRY_PARSE_JSON(c.extra_json) AS extra")
            columns.append(Column("extra", "VARIANT"))
            continue
        items.append(f"c.{name}")
        audits = ("not_null", "unique") if index == 0 else ()
        columns.append(Column(name, model_type(column_type), audits, nullable=index != 0))
    items.extend(
        [
            "c._seq",
            "c._loaded_at",
            "TRY_TO_NUMBER(c._meta:schema_version::VARCHAR)::INT AS _schema_version",
            "COALESCE(c._meta, OBJECT_CONSTRUCT())::OBJECT AS _meta",
        ]
    )
    columns.extend(
        [
            Column("_seq", "NUMBER(38, 0)"),
            Column("_loaded_at", "TIMESTAMP_NTZ"),
            Column("_schema_version", "INT"),
            Column("_meta", "OBJECT"),
        ]
    )
    ctes.append(cte("final", f"SELECT\n{select_list(items)}\nFROM {relation} AS c"))
    kind = "append-only feed" if archetype.append else "change log"
    return project.add(
        Model(
            name=f"base_{source}",
            folder=folder,
            layer="staging",
            description=(
                f"Latest version of each row of the {archetype.description} {kind} from "
                f"{instance.label}"
            ),
            columns=columns,
            body=with_query(ctes),
            sources=[source],
        )
    )


# --- stg_ models ------------------------------------------------------------------------------


def field_expression(item: Field, platform: str, alias: str = "b") -> str:
    if lives_in_payload(item, platform):
        value = f"{alias}.{payload_column(platform)}:{json_path(item, platform)}"
        return json_transform(item.role, value, platform)
    return column_transform(item.role, f"{alias}.{physical_name(item, platform)}", platform)


def _sequence_window(archetype: Archetype, platform: str) -> tuple[str, str] | None:
    """A per-parent sequence number, the row-numbering idiom staging uses for ordering."""
    if platform == "flock":
        return None
    parent = next((item for item in archetype.fields if item.role == "id"), None)
    moment = next(
        (item for item in archetype.fields if item.role == "ts" and not item.in_payload), None
    )
    if parent is None or moment is None:
        return None
    key = physical_name(key_field(archetype), platform)
    name = f"{parent.stem.removesuffix('_id')}_{archetype.table.removesuffix('s')}_number"
    expression = (
        f"ROW_NUMBER() OVER (\n    PARTITION BY b.{physical_name(parent, platform)}\n"
        f"    ORDER BY b.{physical_name(moment, platform)}, b.{key}\n  )::INT AS {name}"
    )
    return name, expression


def stg_model(project: Project, instance: Instance, archetype: Archetype, folder: str) -> Model:
    platform = instance.platform
    source = f"{instance.code}__{archetype.table}"
    key = key_field(archetype)
    items = [
        f"{column_transform('key', f'b.{physical_name(key, platform)}', platform)} AS {key.stem}"
    ]
    columns = [Column(key.stem, "VARCHAR", ("not_null", "unique"), nullable=False)]
    for item in archetype.fields:
        name = staging_name(item, platform)
        items.append(f"{field_expression(item, platform)} AS {name}")
        audits: tuple[str, ...] = ()
        if item.role == "currency":
            audits = ("valid_currency",)
        elif item.role == "qty":
            audits = ("non_negative",)
        columns.append(Column(name, staging_type(item, platform), audits))
    window = _sequence_window(archetype, platform)
    if window is not None:
        items.append(window[1])
        columns.append(Column(window[0], "INT"))
    items.extend(
        [
            "b._schema_version AS source_schema_version",
            "@sql.json_text(b._meta:origin.txid) AS source_txid",
            "COALESCE(b._meta:origin.backfill::BOOLEAN, FALSE) AS is_backfill_row",
            "@sql.load_metadata(b)",
        ]
    )
    columns.extend(
        [
            Column("source_schema_version", "INT"),
            Column("source_txid", "VARCHAR"),
            Column("is_backfill_row", "BOOLEAN"),
            Column("source_seq", "NUMBER(38, 0)"),
            Column("source_loaded_at", "TIMESTAMP_NTZ"),
        ]
    )
    ctes = [
        import_cte("base", f'__ref("base_{source}")'),
        cte("final", f"SELECT\n{select_list(items)}\nFROM base AS b"),
    ]
    return project.add(
        Model(
            name=f"stg_{source}",
            folder=folder,
            layer="staging",
            description=(
                f"One row per live {archetype.description.removesuffix('s')} record from "
                f"{instance.label}, typed and normalised from the {platform} platform layout"
            ),
            columns=columns,
            body=with_query(ctes),
            refs=[f"base_{source}"],
        )
    )


def child_model(
    project: Project, instance: Instance, archetype: Archetype, child: Child, folder: str
) -> Model:
    parent = f"stg_{instance.code}__{archetype.table}"
    platform = instance.platform
    key = archetype.key
    child_key = f"{child.name}_key"
    items = [
        f"(p.{key} || '-' || e.index::VARCHAR)::VARCHAR AS {child_key}",
        f"p.{key}",
        "e.index::INT AS element_index",
    ]
    columns = [
        Column(child_key, "VARCHAR", ("not_null", "unique"), nullable=False),
        Column(key, "VARCHAR", ("not_null",)),
        Column("element_index", "INT"),
    ]
    for item in child.fields:
        payload_item = Field(item.stem, item.role, True)
        name = staging_name(payload_item, "nest")
        items.append(f"{json_transform(item.role, f'e.value:{item.stem}', 'nest')} AS {name}")
        columns.append(Column(name, staging_type(payload_item, "nest")))
    array = staging_name(next(x for x in archetype.fields if x.stem == child.array), platform)
    ctes = [
        import_cte("parent", f'__ref("{parent}")'),
        cte(
            "final",
            f"SELECT\n{select_list(items)}\nFROM parent AS p, LATERAL FLATTEN(input =>"
            f" p.{array}) AS e",
        ),
    ]
    return project.add(
        Model(
            name=f"{parent}__{child.name}",
            folder=folder,
            layer="staging",
            description=f"One row per element of the {child.description} array on {parent}",
            columns=columns,
            body=with_query(ctes),
            refs=[parent],
        )
    )


def generic_child_model(
    project: Project, instance: Instance, archetype: Archetype, item: Field, folder: str
) -> Model:
    """Array elements as text, or object entries as key and value pairs, one row each."""
    parent = f"stg_{instance.code}__{archetype.table}"
    key = archetype.key
    child_key = f"{item.stem}_key"
    column = staging_name(item, instance.platform)
    if item.role == "array":
        position = "e.index"
        detail = [
            "e.index::INT AS element_index",
            "@sql.json_text(e.value) AS element_value",
            "TYPEOF(e.value)::VARCHAR AS element_type",
        ]
        detail_columns = [
            Column("element_index", "INT"),
            Column("element_value", "VARCHAR"),
            Column("element_type", "VARCHAR"),
        ]
        noun = "element"
    else:
        position = "e.key"
        detail = [
            "LOWER(e.key)::VARCHAR AS attribute_key",
            "@sql.json_text(e.value) AS attribute_value",
            "IS_OBJECT(e.value)::BOOLEAN AS is_nested",
        ]
        detail_columns = [
            Column("attribute_key", "VARCHAR"),
            Column("attribute_value", "VARCHAR"),
            Column("is_nested", "BOOLEAN"),
        ]
        noun = "entry"
    items = [
        f"(p.{key} || '-' || {position}::VARCHAR)::VARCHAR AS {child_key}",
        f"p.{key}",
        *detail,
    ]
    columns = [
        Column(child_key, "VARCHAR", ("not_null", "unique"), nullable=False),
        Column(key, "VARCHAR", ("not_null",)),
        *detail_columns,
    ]
    ctes = [
        import_cte("parent", f'__ref("{parent}")'),
        cte(
            "final",
            f"SELECT\n{select_list(items)}\nFROM parent AS p, LATERAL FLATTEN(input =>"
            f" p.{column}) AS e",
        ),
    ]
    return project.add(
        Model(
            name=f"{parent}__{item.stem}",
            folder=folder,
            layer="staging",
            description=(
                f"One row per {noun} of {item.stem.replace('_', ' ')} on {parent}, flattened "
                "from the semi-structured column"
            ),
            columns=columns,
            body=with_query(ctes),
            refs=[parent],
        )
    )


def history_model(project: Project, instance: Instance, archetype: Archetype, folder: str) -> Model:
    """Every version of each key with its validity window, for slowly changing reports."""
    source = f"{instance.code}__{archetype.table}"
    platform = instance.platform
    key_physical = physical_name(key_field(archetype), platform)
    tracked = [item for item in archetype.fields if item.role in ("code", "status", "amount")][:3]
    items = [
        f"{column_transform('key', f'h.{key_physical}', platform)} AS {archetype.key}",
        "h._seq::NUMBER(38, 0) AS version_seq",
        '(h._op = @enum("cdc_op").DELETE)::BOOLEAN AS is_deleted',
        "h._loaded_at::TIMESTAMP_NTZ AS valid_from_loaded_at",
        "h.next_loaded_at::TIMESTAMP_NTZ AS valid_to_loaded_at",
        "(h.next_loaded_at IS NULL)::BOOLEAN AS is_current",
    ]
    columns = [
        Column(archetype.key, "VARCHAR", ("not_null",)),
        Column("version_seq", "NUMBER(38, 0)", ("not_null", "unique"), nullable=False),
        Column("is_deleted", "BOOLEAN"),
        Column("valid_from_loaded_at", "TIMESTAMP_NTZ"),
        Column("valid_to_loaded_at", "TIMESTAMP_NTZ"),
        Column("is_current", "BOOLEAN"),
    ]
    for item in tracked:
        name = staging_name(item, platform)
        items.append(f"{field_expression(item, platform, 'h')} AS {name}")
        columns.append(Column(name, staging_type(item, platform)))
    needs_payload = platform == "classic" and any(item.in_payload for item in tracked)
    versions = ["s.*", "TRY_PARSE_JSON(s.extra_json) AS extra"] if needs_payload else ["s.*"]
    versions.append(
        f"LEAD(s._loaded_at) OVER (PARTITION BY s.{key_physical} ORDER BY s._seq) AS next_loaded_at"
    )
    ctes = [
        import_cte("src", f'__source("{source}")'),
        f"@sql.cdc_history(src, src.key := ({key_physical}))",
        cte("versions", f"SELECT\n{select_list(versions)}\nFROM cdc_history AS s"),
        cte("final", f"SELECT\n{select_list(items)}\nFROM versions AS h"),
    ]
    return project.add(
        Model(
            name=f"stg_{source}__history",
            folder=folder,
            layer="staging",
            description=(
                f"Every version of each {archetype.description.removesuffix('s')} record from "
                f"{instance.label}, with the load times it was valid between"
            ),
            columns=columns,
            body=with_query(ctes),
            sources=[source],
        )
    )


def stage_instance(project: Project, instance: Instance, folder: str) -> list[StagedTable]:
    yaml_entries: list[str] = []
    staged: list[StagedTable] = []
    for archetype in ARCHETYPES[instance.system]:
        yaml_entries.append(source_yaml(instance, archetype))
        base_model(project, instance, archetype, folder)
        stg = stg_model(project, instance, archetype, folder)
        children = {
            child.name: child_model(project, instance, archetype, child, folder).name
            for child in archetype.children
        }
        for item in archetype.fields:
            if item.role in ("array", "object") and item.stem not in {
                child.array for child in archetype.children
            }:
                children[item.stem] = generic_child_model(
                    project, instance, archetype, item, folder
                ).name
        history = (
            history_model(project, instance, archetype, folder).name
            if archetype.history and not archetype.append
            else None
        )
        staged.append(StagedTable(instance, archetype, stg.name, children, history))
    project.write(
        f"sql/sources/{instance.code}.yml",
        f"# Raw tables from {instance.label}.\nsources:\n" + "\n".join(yaml_entries) + "\n",
    )
    return staged


# --- Clickstream ------------------------------------------------------------------------------

EVENT_COLUMNS: tuple[tuple[str, str], ...] = (
    ("event_id", "VARCHAR"),
    ("received_at", "TIMESTAMP_NTZ"),
    ("event_type", "VARCHAR"),
    ("event_at_utc", "TIMESTAMP_NTZ"),
    ("anonymous_id", "VARCHAR"),
    ("customer_id", "VARCHAR"),
    ("storefront_code", "VARCHAR"),
    ("page_path", "VARCHAR"),
    ("referrer_host", "VARCHAR"),
    ("client_timezone", "VARCHAR"),
    ("is_bot", "BOOLEAN"),
    ("properties", "OBJECT"),
    ("items", "ARRAY"),
)


def stage_collector(project: Project, instance: Instance, folder: str) -> tuple[str, list[str]]:
    source = f"{instance.code}__events"
    project.write(
        f"sql/sources/{instance.code}.yml",
        "\n".join(
            [
                f"# Raw events from the {instance.label} collector.",
                "sources:",
                f"  - name: {source}",
                f"    description: Tracker events received by the {instance.label} collector",
                f"    database: {RAW_DATABASE}",
                f"    schema: {instance.code.upper()}",
                "    table: EVENTS",
                "    contract: enforced",
                "    columns:",
                "      - name: event_id",
                "        type: VARCHAR",
                "        nullable: false",
                "        audits:",
                "          - not_null",
                "      - name: received_at",
                "        type: TIMESTAMP_NTZ",
                "        nullable: false",
                "      - name: event",
                "        type: VARIANT",
                "      - name: _seq",
                "        type: NUMBER(38,0)",
                "        nullable: false",
                "        audits:",
                "          - not_null",
                "          - unique",
                "      - name: _loaded_at",
                "        type: TIMESTAMP_NTZ",
                "        nullable: false",
                "",
            ]
        ),
    )
    base = project.add(
        Model(
            name=f"base_{source}",
            folder=folder,
            layer="staging",
            description=f"Each tracker event from the {instance.label} collector once",
            columns=[
                Column("event_id", "VARCHAR", ("not_null", "unique"), nullable=False),
                Column("received_at", "TIMESTAMP_NTZ", ("not_null",)),
                Column("event", "VARIANT"),
                Column("_seq", "NUMBER(38, 0)"),
                Column("_loaded_at", "TIMESTAMP_NTZ"),
            ],
            body=with_query(
                [
                    import_cte("src", f'__source("{source}")'),
                    cte(
                        "final",
                        "SELECT\n  s.event_id,\n  s.received_at,\n  s.event,\n  s._seq,\n"
                        "  s._loaded_at\nFROM src AS s\n"
                        "QUALIFY ROW_NUMBER() OVER (PARTITION BY s.event_id ORDER BY"
                        " s._seq DESC) = 1",
                    ),
                ]
            ),
            sources=[source],
        )
    )
    items = [
        "b.event_id",
        "b.received_at",
        "LOWER(REGEXP_REPLACE(b.event:type::VARCHAR, '[^A-Za-z]+', '_'))::VARCHAR AS event_type",
        "COALESCE(\n    @sql.epoch_ms_to_utc(b.event:ts_ms::NUMBER(38, 0)),\n    b.received_at\n  )"
        "::TIMESTAMP_NTZ AS event_at_utc",
        "LOWER(b.event:anonymous_id::VARCHAR)::VARCHAR AS anonymous_id",
        "@sql.json_text(b.event:user.id) AS customer_id",
        "UPPER(b.event:context.storefront::VARCHAR)::VARCHAR AS storefront_code",
        "('/' || LTRIM(PARSE_URL(b.event:page.url::VARCHAR, 1)['path']::VARCHAR, '/'))"
        "::VARCHAR AS page_path",
        "LOWER(PARSE_URL(b.event:page.referrer::VARCHAR, 1)['host']::VARCHAR)::VARCHAR"
        " AS referrer_host",
        "b.event:context.tz::VARCHAR AS client_timezone",
        "COALESCE(REGEXP_LIKE(\n    b.event:context.ua::VARCHAR,\n   "
        ' @const("bot_user_agent_pattern"),'
        "\n    'i'\n  ), FALSE)::BOOLEAN AS is_bot",
        "COALESCE(b.event:properties, OBJECT_CONSTRUCT())::OBJECT AS properties",
        "COALESCE(b.event:items, ARRAY_CONSTRUCT())::ARRAY AS items",
    ]
    columns = [Column(name, column_type) for name, column_type in EVENT_COLUMNS]
    columns[0] = Column(
        "event_id", "VARCHAR", ("not_null (run_scope delta_and_final)", "unique"), nullable=False
    )
    columns[1] = Column("received_at", "TIMESTAMP_NTZ", ("not_null",))
    events = project.add(
        Model(
            name=f"stg_{source}",
            folder=folder,
            layer="staging",
            description=(
                f"One row per tracker event from the {instance.label} collector, typed"
                " from the raw "
                "payload and built in six-hour microbatches. An event without a client timestamp "
                "takes the time the collector received it."
            ),
            config=[
                "materialized incremental",
                "incremental_strategy delete_insert",
                "incremental_mode microbatch",
                "microbatch_strategy watermark",
                "cursor_watermark_mode all",
                "cursor received_at",
                "cursor_type timestamp",
                "cursor_grain hour",
                "batch_size 6h",
                "lookback 2h",
                "replay_on_change full",
                "cursor_inputs (\n    "
                f"{base.name} (column received_at, roles [filter, watermark]),\n  )",
                "on_schema_change append_new_columns",
            ],
            columns=columns,
            body=with_query(
                [
                    import_cte("base", f'__ref("{base.name}")'),
                    cte("final", f"SELECT\n{select_list(items)}\nFROM base AS b"),
                ]
            ),
            refs=[base.name],
        )
    )
    typed: list[str] = []
    for event_type, fields in EVENT_TYPES:
        typed.append(event_model(project, instance, events, event_type, fields, folder).name)
    return events.name, typed


def event_model(
    project: Project,
    instance: Instance,
    events: Model,
    event_type: str,
    fields: tuple[Field, ...],
    folder: str,
) -> Model:
    items = [
        "e.event_id",
        "e.event_at_utc",
        "e.anonymous_id",
        "e.customer_id",
        "e.storefront_code",
        "e.page_path",
    ]
    columns = [
        Column("event_id", "VARCHAR", ("not_null", "unique"), nullable=False),
        Column("event_at_utc", "TIMESTAMP_NTZ", ("not_null",)),
        Column("anonymous_id", "VARCHAR"),
        Column("customer_id", "VARCHAR"),
        Column("storefront_code", "VARCHAR"),
        Column("page_path", "VARCHAR"),
    ]
    for item in fields:
        name = staging_name(item, "nest")
        items.append(f"{json_transform(item.role, f'e.properties:{item.stem}', 'nest')} AS {name}")
        columns.append(Column(name, staging_type(item, "nest")))
    body = with_query(
        [
            import_cte("events", f'__ref("{events.name}")'),
            cte(
                "final",
                f"SELECT\n{select_list(items)}\nFROM events AS e\n"
                f"WHERE\n  e.event_type = '{event_type}'\n  AND NOT e.is_bot",
            ),
        ]
    )
    return project.add(
        Model(
            name=f"stg_{instance.code}__{event_type}_events",
            folder=folder,
            layer="staging",
            description=(
                f"Human {event_type.replace('_', ' ')} events from the {instance.label} collector "
                "with their typed properties"
            ),
            columns=columns,
            body=body,
            refs=[events.name],
        )
    )

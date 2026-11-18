"""The warehouse state a location syncs after building the DuckShop 10K project, as mirror batches.

usage: synced_state.py <control.sqlite3> <plan_output capture> <out.json>
       [--built-share 1.0] [--changed-share 0.01] [--seed 20261116]

The plan preview guard plans the generated project from a synced copy of a warehouse that built it.
No warehouse runs the project, so this writes what one would have recorded: a relation, a
fingerprint and two `--changed` executions per built model, an execution per seed, cursor
observations of incremental models, the source tables and a few stray relations.

The fingerprints and executions carry the hashes the planner computes for the project as published,
read from a `SQLBUILD_PLANNER_CAPTURE_DIR` capture of one plan of it, so an unedited model plans as
unchanged and `--changed` finds only the edits: `--changed-share` of the built models record a
definition one line older, and the models outside `--built-share` were never built.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import random
import sqlite3
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

STATE_DATABASE: str = "DUCKSHOP_10K"
DIALECT: str = "snowflake"
STATE_SCHEMA: str = "DEV"
SYNCED_AT: datetime = datetime(2026, 11, 16, 6, 0, tzinfo=UTC)
NODE_KINDS: tuple[str, ...] = ("model", "source", "seed", "udf", "table_fn")
FUNCTION_KINDS: frozenset[str] = frozenset({"udf", "table_fn"})
STATE_TABLES: tuple[str, ...] = (
    "_sqlbuild_fingerprints",
    "_sqlbuild_consumption",
    "_sqlbuild_observations",
    "_sqlbuild_microbatches",
    "_sqlbuild_source_freshness",
)
STRAY_RELATIONS: int = 150
EXECUTIONS: int = 2
RUN_IDS: tuple[str, ...] = tuple(
    f"01a1d{index:03x}-0000-7000-8000-{index:012x}" for index in range(40)
)


def iso(moment: datetime) -> str:
    return moment.isoformat().replace("+00:00", "Z")


def digest(*parts: str) -> str:
    return hashlib.sha256("\x1f".join(parts).encode()).hexdigest()


def canonical(value: dict[str, Any]) -> str:
    """JSON with sorted keys and no spaces, as the planner hashes identity metadata."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def published_nodes(control: Path) -> dict[str, dict[str, Any]]:
    """Every model, source, seed and function of the current snapshot, by node key."""
    with sqlite3.connect(control) as connection:
        rows = connection.execute(
            "SELECT n.node_key, d.body FROM sqb_snapshot_nodes n "
            "JOIN sqb_current_snapshots c ON c.snapshot_id=n.snapshot_id "
            "JOIN sqb_node_definitions d ON d.node_hash=n.node_hash "
            f"WHERE n.node_kind IN ({','.join('?' * len(NODE_KINDS))})",
            NODE_KINDS,
        ).fetchall()
    return {key: json.loads(gzip.decompress(body))["definition"] for key, body in rows}


def planned_identities(capture: Path) -> dict[str, dict[str, str]]:
    """Each planned model's and seed's fingerprint fields and `--changed` version hash, by node key.

    A model's `--changed` version hash is its local identity hash: the query fingerprint and the
    identity metadata without the declared columns, which change detection adds back. A plan of a
    project never built records no query fingerprint, so it is computed as the planner does.
    """
    from sqlbuild._native import compute_query_hash

    document: dict[str, Any] = json.loads(capture.read_text())
    shared: dict[str, Any] = document["__shared_nodes__"]

    def resolved(value: Any) -> Any:
        while isinstance(value, dict) and set(value) == {"__shared__"}:
            value = shared[value["__shared__"]]
        return value

    found: dict[str, dict[str, str]] = {}
    pending: list[Any] = [document]
    while pending:
        value: Any = resolved(pending.pop())
        if isinstance(value, list):
            pending.extend(value)
            continue
        if not isinstance(value, dict):
            continue
        entry_type: str = str(value.get("__type__", ""))
        if entry_type.endswith((":ModelPlanEntry", ":SeedPlanEntry")) and "name" in value:
            kind: str = "model" if entry_type.endswith(":ModelPlanEntry") else "seed"
            metadata_json: str = resolved(value["fingerprint_metadata_json"])
            definition: str = str(
                resolved(
                    value.get(
                        "fingerprint_query_sql" if kind == "model" else "fingerprint_definition"
                    )
                )
                or ""
            )
            definition_hash: str = str(resolved(value.get("fingerprint_definition_hash")) or "")
            if kind == "model" and not definition_hash:
                definition_hash = compute_query_hash(query_sql=definition, dialect=DIALECT)
            identity: dict[str, Any] = json.loads(metadata_json)
            identity.pop("declared_columns_hash", None)
            local: str = hashlib.sha256(
                f"{definition_hash}\n{canonical(identity)}".encode()
            ).hexdigest()
            version_hash: str = resolved(value["fingerprint_version_hash"])
            definition_hash = definition_hash or version_hash
            found[f"{kind}:{resolved(value['name'])}"] = {
                "definition_hash": definition_hash,
                "version_hash": version_hash,
                "metadata_json": metadata_json,
                "definition": definition,
                "local_hash": local if kind == "model" else version_hash,
            }
            continue
        pending.extend(value.values())
    return found


def inputs(node: dict[str, Any], nodes: dict[str, dict[str, Any]]) -> list[tuple[str, str]]:
    """The inputs `--changed` reads for a node: upstream nodes, sources, and through functions."""
    found: list[tuple[str, str]] = []
    seen: set[str] = set()
    pending: list[str] = list(node["deps"])
    while pending:
        dep: str = pending.pop(0)
        if dep in seen:
            continue
        seen.add(dep)
        kind, name = dep.split(":", 1)
        if kind in ("model", "seed"):
            found.append(("node", dep))
        elif kind == "source":
            found.append(("source", name))
        elif kind in FUNCTION_KINDS and dep in nodes:
            pending.extend(nodes[dep]["deps"])
        else:
            found.append(("opaque", dep))
    return sorted(found)


def upstream_first(keys: list[str], nodes: dict[str, dict[str, Any]]) -> list[str]:
    """`keys` with every node after the built nodes it reads, so each execution names theirs."""
    wanted: set[str] = set(keys)
    ordered: list[str] = []
    seen: set[str] = set()
    for start in sorted(keys):
        stack: list[tuple[str, bool]] = [(start, False)]
        while stack:
            key, done = stack.pop()
            if done:
                ordered.append(key)
                continue
            if key in seen:
                continue
            seen.add(key)
            stack.append((key, True))
            stack.extend(
                (dep, False) for dep in nodes[key]["deps"] if dep in wanted and dep not in seen
            )
    return ordered


def relation(
    name: str, relation_type: str, columns: list[dict[str, Any]], location: tuple[str, str]
) -> dict[str, Any]:
    database, schema = location
    return {
        "state_database": database,
        "state_schema": schema,
        "name": name,
        "relation_type": relation_type,
        "columns": [
            {"name": column["name"].upper(), "data_type": column["type"] or "VARCHAR"}
            for column in columns
        ],
    }


def mirror_batches(
    nodes: dict[str, dict[str, Any]],
    identities: dict[str, dict[str, str]],
    *,
    built_share: float,
    changed_share: float,
    seed: int,
) -> dict[str, list[dict[str, Any]]]:
    """The catalog, fingerprint, consumption and observation records of the built warehouse."""
    rng: random.Random = random.Random(seed)
    state: tuple[str, str] = (STATE_DATABASE, STATE_SCHEMA)
    models: list[str] = sorted(key for key in nodes if key.startswith("model:"))
    built: list[str] = [key for key in models if rng.random() < built_share]
    seeds: list[str] = sorted(key for key in nodes if key.startswith("seed:"))
    catalog: list[dict[str, Any]] = []
    fingerprints: list[dict[str, Any]] = []
    consumption: list[dict[str, Any]] = []
    observations: list[dict[str, Any]] = []
    for key in sorted(nodes):
        kind, name = key.split(":", 1)
        node: dict[str, Any] = nodes[key]
        if kind == "source":
            location = (node["database"] or STATE_DATABASE, node["schema"] or STATE_SCHEMA)
            catalog.append(
                relation((node["table"] or name).upper(), "TABLE", node["columns"], location)
            )
        elif kind == "seed":
            catalog.append(relation(name.upper(), "TABLE", node["columns"], state))
    latest_execution: dict[str, str] = {}
    for position, key in enumerate([*seeds, *upstream_first(built, nodes)]):
        kind, name = key.split(":", 1)
        node = nodes[key]
        identity: dict[str, str] = identities[key]
        run_id: str = RUN_IDS[position % len(RUN_IDS)]
        built_at: datetime = SYNCED_AT - timedelta(hours=2) + timedelta(milliseconds=position * 50)
        edited: bool = kind == "model" and rng.random() < changed_share
        local_hash: str = digest("edited", name) if edited else identity["local_hash"]
        fingerprints.append(
            {
                "event_id": digest("event", key)[:32],
                "state_database": STATE_DATABASE,
                "state_schema": STATE_SCHEMA,
                "node_type": kind,
                "node_name": name,
                "target_database": STATE_DATABASE,
                "target_schema": STATE_SCHEMA,
                "target_name": name,
                "run_id": run_id,
                "definition_hash": digest("edited definition", name)
                if edited
                else identity["definition_hash"],
                "version_hash": digest("edited version", name)
                if edited
                else identity["version_hash"],
                "schema_fingerprint": digest(
                    "schema", name, *[column["name"] for column in node["columns"]]
                ),
                "definition": identity["definition"].replace("SELECT", "SELECT DISTINCT", 1)
                if edited
                else identity["definition"],
                "metadata_json": identity["metadata_json"],
                "ts": iso(built_at),
            }
        )
        if kind == "model":
            materialization: str = node["materialization"]
            catalog.append(
                relation(
                    name.upper(),
                    "VIEW" if materialization == "view" else "TABLE",
                    node["columns"],
                    state,
                )
            )
            if materialization == "incremental":
                observations.append(
                    {
                        "state_database": STATE_DATABASE,
                        "state_schema": STATE_SCHEMA,
                        "observation_id": digest("cursor", name)[:32],
                        "kind": "cursor_max",
                        "node": name,
                        "cursor_type": node["config"].get("cursor_type", "timestamp"),
                        "value": iso(built_at - timedelta(minutes=5)),
                        "run_id": run_id,
                        "observed_at": iso(built_at),
                    }
                )
        read: list[tuple[str, str]] = inputs(node, nodes) if kind == "model" else []
        for execution in reversed(range(EXECUTIONS)):
            recorded_at: datetime = built_at - timedelta(days=execution)
            row: dict[str, Any] = {
                "state_database": STATE_DATABASE,
                "state_schema": STATE_SCHEMA,
                "execution_id": digest("execution", key, str(execution))[:32],
                "run_id": run_id,
                "node_type": kind,
                "node_name": name,
                "target_database": STATE_DATABASE,
                "target_schema": STATE_SCHEMA,
                "target_name": name,
                "version_hash": local_hash,
                "input_count": len(read),
            }
            consumption.append(
                row
                | {
                    "phase": "started",
                    "input_kind": None,
                    "input_name": None,
                    "input_version": None,
                    "recorded_at": iso(recorded_at),
                }
            )
            succeeded_at: str = iso(recorded_at + timedelta(milliseconds=20))
            rows: list[tuple[str | None, str | None]] = [(kind, name) for kind, name in read]
            if not rows:
                rows.append((None, None))
            for input_kind, input_name in rows:
                version: str | None = None
                if input_kind is not None and input_name is not None:
                    version = (
                        latest_execution.get(input_name, "unknown")
                        if input_kind == "node"
                        else "unknown"
                    )
                consumption.append(
                    row
                    | {
                        "phase": "succeeded",
                        "input_kind": input_kind,
                        "input_name": input_name,
                        "input_version": version,
                        "recorded_at": succeeded_at,
                    }
                )
        latest_execution[key] = digest("execution", key, "0")[:32]
    for table in STATE_TABLES:
        catalog.append(relation(table, "TABLE", [{"name": "run_id", "type": "VARCHAR"}], state))
    for index in range(STRAY_RELATIONS):
        catalog.append(
            relation(
                f"LEGACY_REPORT_{index:04d}",
                "VIEW",
                [{"name": "id", "type": "NUMBER(38,0)"}, {"name": "value", "type": "VARCHAR"}],
                state,
            )
        )
    return {
        "catalog": catalog,
        "fingerprints": fingerprints,
        "consumption": consumption,
        "observations": observations,
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("control", type=Path)
    parser.add_argument("capture", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--built-share", type=float, default=1.0)
    parser.add_argument("--changed-share", type=float, default=0.01)
    parser.add_argument("--seed", type=int, default=20261116)
    options = parser.parse_args()
    batches = mirror_batches(
        published_nodes(options.control),
        planned_identities(options.capture),
        built_share=options.built_share,
        changed_share=options.changed_share,
        seed=options.seed,
    )
    options.out.write_text(json.dumps(batches))
    print(json.dumps({kind: len(records) for kind, records in batches.items()}))


if __name__ == "__main__":
    main()

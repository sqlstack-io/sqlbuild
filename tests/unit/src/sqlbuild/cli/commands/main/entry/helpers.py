"""Test helpers for CLI entry tests."""

from __future__ import annotations

import importlib.util
import signal
import weakref
from collections.abc import Callable
from importlib.machinery import ModuleSpec
from pathlib import Path
from types import ModuleType
from typing import Any, Protocol, cast

from sqlbuild._native import (
    CliEntrypointHandlers,
    CompileCommandRequest,
    SpecConfigError,
    loader_destination_parts,
)


class JsonOutputRequest(Protocol):
    json_output: bool
    json_output_path: Path | None


class UnprintableError(Exception):
    """A handler error whose message raises when read."""

    def __str__(self) -> str:
        """Fail as a broken `__str__` does."""
        raise RuntimeError("unreadable")


def invalid_loader_destination_error(_: Path) -> SpecConfigError:
    """Return the contract error produced by an invalid loader destination."""

    try:
        loader_destination_parts(
            destination="raw..orders", default_database=None, default_schema=None
        )
    except SpecConfigError as error:
        return error
    raise AssertionError("invalid loader destination was accepted")


def noop_handler(*_a: Any, **_k: Any) -> int:
    return 0


def compile_with_callback_interrupt(_request: CompileCommandRequest) -> int:
    """Deliver one real SIGINT from a callback where Python cannot propagate its exception."""

    def value() -> None:
        return None

    reference: weakref.ReferenceType[Callable[[], None]] = weakref.ref(
        value, lambda _: signal.raise_signal(signal.SIGINT)
    )
    del value
    assert reference() is None
    return 0


def extract_json_output_fields(args: tuple[object, ...]) -> tuple[bool, Path | None]:
    """Return (json_output, json_output_path) from a typed request or positional args."""

    request: JsonOutputRequest = cast(JsonOutputRequest, args[0])
    return bool(request.json_output), request.json_output_path


def build_json_recording_handler(
    *, received_args: list[tuple[bool, Path | None]], exit_code: int
) -> Callable[..., int]:
    """Build a fake command handler that records json output flags and returns exit_code."""

    def record_json_handler(*args: object, **kwargs: object) -> int:
        received_args.append(extract_json_output_fields((*args, *kwargs.values())))
        return exit_code

    return record_json_handler


def build_handlers(**overrides: Any) -> CliEntrypointHandlers:
    """Build handlers with noop defaults, overriding specific slots."""

    return CliEntrypointHandlers(
        run_compile=overrides.get("run_compile", noop_handler),
        run_cost=overrides.get("run_cost", noop_handler),
        run_dag=overrides.get("run_dag", noop_handler),
        run_plan=overrides.get("run_plan", noop_handler),
        run_dbt_plan=overrides.get("run_dbt_plan", noop_handler),
        run_dbt_run=overrides.get("run_dbt_run", noop_handler),
        run_dbt_build=overrides.get("run_dbt_build", noop_handler),
        run_dbt_debug=overrides.get("run_dbt_debug", noop_handler),
        run_dbt_init=overrides.get("run_dbt_init", noop_handler),
        run_build=overrides.get("run_build", noop_handler),
        run_freshness=overrides.get("run_freshness", noop_handler),
        run_test=overrides.get("run_test", noop_handler),
        run_check=overrides.get("run_check", noop_handler),
        run_audit=overrides.get("run_audit", noop_handler),
        run_seed=overrides.get("run_seed", noop_handler),
        run_load=overrides.get("run_load", noop_handler),
        run_clone=overrides.get("run_clone", noop_handler),
        run_diff=overrides.get("run_diff", noop_handler),
        run_query=overrides.get("run_query", noop_handler),
        run_debug=overrides.get("run_debug", noop_handler),
        run_lineage=overrides.get("run_lineage", noop_handler),
        run_janitor=overrides.get("run_janitor", noop_handler),
        run_init=overrides.get("run_init", noop_handler),
        run_playground=overrides.get("run_playground", noop_handler),
        run_skills_update=overrides.get("run_skills_update", noop_handler),
        run_format=overrides.get("run_format", noop_handler),
        run_scenario=overrides.get("run_scenario", noop_handler),
        run_scenario_capture=overrides.get("run_scenario_capture", noop_handler),
        run_rules=overrides.get("run_rules", noop_handler),
        run_scope=overrides.get("run_scope", noop_handler),
    )


PROJECT_LOGGING_MODULE: str = """import logging


def log_user_messages() -> None:
    logger = logging.getLogger("project.cli")
    prior_level = logger.level
    logger.setLevel(logging.DEBUG)
    logger.debug("CLI user debug")
    logger.info("CLI user info")
    logger.setLevel(prior_level)
"""


def load_project_module(path: Path) -> ModuleType:
    """Import a project file so its log records come from inside the project directory."""

    spec: ModuleSpec | None = importlib.util.spec_from_file_location(path.stem, path)
    assert spec is not None
    assert spec.loader is not None
    module: ModuleType = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

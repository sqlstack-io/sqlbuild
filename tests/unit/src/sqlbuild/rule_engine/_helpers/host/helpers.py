"""Builders for custom-rule host partitioning tests."""

import os
import random
import shutil
import time
from collections.abc import Callable
from operator import attrgetter
from pathlib import Path
from types import SimpleNamespace
from typing import Protocol

import pytest

import sqlbuild._native as native_module
from sqlbuild._native import (
    CustomHostObserver,
    CustomRulesOutcome,
    OpaqueModuleStateError,
    Rule,
    RulesCacheConfig,
    RulesConfig,
    build_rule_catalogue,
    project_with,
)
from sqlbuild._native import evaluate_custom_rules as evaluate_custom_rules_cached
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.rule_engine._helpers.host.module_state import module_state_token, rule_namespaces
from sqlbuild.rule_engine.classes.module_state_monitor import ModuleStateMonitor
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project

_MODEL_RULE: str = "XSQBRT101"
_PROJECT_RULE: str = "XSQBRT102"
_RULES_MODULE: str = f'''from sqlbuild.rules import Finding, Model, Project, RuleContext, rule


@rule(code="{_MODEL_RULE}", message="order models need an even suffix", remediation="Rename it.")
def even_suffix(*, model: Model, ctx: RuleContext) -> list[Finding]:
    if STARTED_DIR:
        os.mkdir(Path(STARTED_DIR) / f"{{model.name}}.{{os.getpid()}}")
    if model.name in FAILING:
        raise ValueError(f"cannot inspect {{model.name}}")
    time.sleep(DELAY_SECONDS)
    if RECORD_SEEN:
        SEEN[model.name] = True
    source: str = ctx.sql.for_model(model).expanded.source
    parity: str = "odd" if "odd" in source else "even"
    if DETECT_DUPLICATES:
        duplicate: bool = parity in SEEN
        SEEN[parity] = True
        return [ctx.finding(subject=model)] if duplicate else []
    return [ctx.finding(subject=model)] if parity == "odd" else []


@rule(code="{_PROJECT_RULE}", message="projects need few models", remediation="Split it.")
def few_models(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    first: Model = ctx.project.models[0]
    return [ctx.finding(subject=first)] if len(ctx.project.models) > 3 else []
'''
_PARITIES: tuple[str, str] = ("even", "odd")


def write_order_rules(
    *,
    root: Path,
    failing: tuple[str, ...] = (),
    records_module_state: bool = False,
    detects_duplicates: bool = False,
    delay_seconds: float = 0.0,
    started_dir: Path | str = "",
) -> None:
    """Write one model rule and one project rule with optional failures, state, or delays.

    With `started_dir`, each model invocation first creates the folder `<model>.<host pid>` there;
    the host's hermeticity guard refuses opening files but allows creating folders."""

    rules: Path = root / "rules" / "orders.py"
    rules.parent.mkdir(parents=True, exist_ok=True)
    rules.write_text(
        "import os\nimport time\nfrom pathlib import Path\n\n"
        f"STARTED_DIR = {str(started_dir)!r}\n"
        f"FAILING = {set(failing)!r}\nRECORD_SEEN = {records_module_state!r}\n"
        f"DETECT_DUPLICATES = {detects_duplicates!r}\nDELAY_SECONDS = {delay_seconds!r}\n"
        f"SEEN: dict[str, bool] = {{}}\n{_RULES_MODULE}",
        encoding="utf-8",
    )


def orders_project(*, model_count: int, edits: dict[str, str] | None = None) -> CompiledProjectView:
    """Return numbered order models whose SQL marks them odd or even, with optional edits."""

    sql_by_name: dict[str, str] = {
        f"orders_{index:03d}": f"SELECT 1 AS order_id -- {_PARITIES[index % 2]}"
        for index in range(model_count)
    }
    sql_by_name.update(edits or {})
    projects: list[CompiledProjectView] = [
        build_project(name=name, relative_path=f"sql/models/{name}.sql", sql=sql, config_values={})
        for name, sql in sql_by_name.items()
    ]
    return project_with(
        project=projects[0], models=tuple(project.models[0] for project in projects)
    )


def evaluate_order_rules(
    *, project: CompiledProjectView, root: Path, cache_enabled: bool = True
) -> CustomRulesOutcome:
    """Evaluate both order rules through the incremental custom-rule cache."""

    config: RulesConfig = RulesConfig(
        select=(_MODEL_RULE, _PROJECT_RULE), cache=RulesCacheConfig(enabled=cache_enabled)
    )
    rules: tuple[Rule, ...] = tuple(
        filter(
            attrgetter("custom"),
            build_rule_catalogue(config=config, project_dir=root, include_custom=True),
        )
    )
    return evaluate_custom_rules_cached(
        project=project, config=config, project_dir=root, rules=rules, dialect="duckdb"
    )


def use_hosts(
    *,
    monkeypatch: pytest.MonkeyPatch,
    hosts: int,
    publish_inputs: Callable[[Path], object] | None = None,
) -> CustomHostObserver:
    """Allow up to `hosts` host processes for any plan and record every host launch."""

    observer: CustomHostObserver = CustomHostObserver(
        cores=hosts, min_invocations_per_host=1, publish_inputs=publish_inputs
    )
    monkeypatch.setattr(native_module, "custom_host_observer", observer)
    return observer


def host_inputs_left(root: Path) -> tuple[Path, ...]:
    """Return every payload, marker or folder a run left in the host-input folder."""

    return tuple((root / "target" / "rules-cache" / "host-inputs").iterdir())


def child_processes() -> frozenset[int]:
    """Return the live child processes of this process."""

    children: set[int] = set()
    for task in (Path("/proc") / str(os.getpid()) / "task").iterdir():
        try:
            listed: str = (task / "children").read_text()
        except FileNotFoundError:
            continue  # the thread exited after the task list was read
        children.update(map(int, listed.split()))
    return frozenset(children)


def delayed_publish(delay_seconds: float) -> Callable[[Path], None]:
    """Return a publish hook that lets the payload be written only after a delay."""

    def delay(path: Path) -> None:
        del path
        time.sleep(delay_seconds)

    return delay


def failed_publish(delay_seconds: float) -> Callable[[Path], None]:
    """Return a publish hook that fails to write the payload after a delay."""

    def fail(path: Path) -> None:
        del path
        time.sleep(delay_seconds)
        raise OSError("project payload disk is full")

    return fail


def failed_publish_without_directory(
    delay_seconds: float,
) -> tuple[Callable[[Path], None], list[float]]:
    """Return a hook that removes the input folder then fails, and when each failure happened."""

    failures: list[float] = []

    def fail(path: Path) -> None:
        time.sleep(delay_seconds)
        shutil.rmtree(path.parent)
        failures.append(time.monotonic())
        raise OSError("project payload disk is full")

    return fail, failures


def started_hosts(started_dir: Path) -> int:
    """Return how many host processes started an invocation, from `<model>.<host pid>` folders."""

    return len({path.suffix for path in started_dir.iterdir()})


def record_signals(*, monkeypatch: pytest.MonkeyPatch) -> list[tuple[int, int]]:
    """Replace process signalling with a recorder so no process can be signalled."""

    signals: list[tuple[int, int]] = []

    def record(pid: int, signal_number: int) -> None:
        signals.append((pid, signal_number))

    monkeypatch.setattr(os, "kill", record)
    monkeypatch.setattr(os, "killpg", record, raising=False)
    return signals


STATE_RULE_HEADER: str = "from sqlbuild.rules import Finding, Model, RuleContext, rule\n"
IMPORT_ORDER_HELPERS: str = "from rules import helpers\n"
ORDER_HELPERS: str = """import functools
import re

STATUS_LABELS = {f"status_{index:03d}": f"Label {index}" for index in range(40)}
REGION_CODES = tuple(f"region_{index:03d}" for index in range(40))
ALLOWED_PREFIXES = frozenset({"orders", "customers", "products"})
PATTERNS = tuple(re.compile(rf"^orders_[0-9]+{index}$") for index in range(5))
NESTED = {f"group_{index}": tuple((f"key_{key}", key) for key in range(4)) for index in range(5)}
SHARED: dict[str, int] = {}


class Registry:
    entries = []


def register(name: str) -> None:
    Registry.entries.append(name)


@functools.lru_cache(maxsize=None)
def cached_length(name: str) -> int:
    return len(name)


def is_known(name: str) -> bool:
    return name in STATUS_LABELS or any(pattern.match(name) for pattern in PATTERNS)
"""
RAISING_PROXY_LIBRARY: str = """class BoomError(Exception):
    pass


class Proxy:
    def __getattr__(self, name):
        raise BoomError(name)


class Helper:
    proxy = Proxy()


def helper_fn():
    return 1
"""
SIDE_EFFECT_PROPERTY_LIBRARY: str = """CALLS = []


class Lazy:
    @property
    def cache_info(self):
        CALLS.append(1)
        raise AttributeError("cache_info")


class Holder:
    lazy = Lazy()


def lazy_fn():
    return len(CALLS)
"""
FOREIGN_MODULE_CLASS_LIBRARY: str = """class BoomError(Exception):
    pass


class Proxy:
    def __getattr__(self, name):
        raise BoomError(name)


class Hidden:
    proxy = Proxy()


Hidden.__module__ = "rules.cross_b"
"""
STATELESS_RULE_CODE: str = "XSQBRST900"
STATEFUL_RULE_CODE: str = "XSQBRST001"
RANDOM_OPERATIONS: tuple[str, ...] = (
    "_ = helpers.is_known(model.name)",
    "LOCAL[model.name] = 1",
    "ITEMS.append(model.index)",
    "ITEMS.append(1)\nITEMS.pop()",
    "if ITEMS:\n    ITEMS.pop()\nelse:\n    ITEMS.append(1)",
    "MARKS.add(model.name)",
    "Holder.total += 1",
    "BOX.value = model.index",
    "globals()['COUNTER'] += 1",
    "globals()['TABLE'] = dict(TABLE)",
    "NESTED['inner'].append(model.index)",
    "helpers.SHARED[model.name] = 1",
    "helpers.register(model.name)",
    "_ = helpers.cached_length(model.name)",
)
RANDOM_HEADERS: tuple[str, ...] = (
    "from rules.helpers import Registry\n" + IMPORT_ORDER_HELPERS,
    IMPORT_ORDER_HELPERS + "from rules.helpers import Registry\n",
)
RANDOM_STATE: str = (
    "LOCAL: dict[str, int] = {}\nITEMS: list[int] = []\nMARKS: set[str] = set()\n"
    "COUNTER = 0\nTABLE = {'orders': 1}\nNESTED: dict[str, list[int]] = {'inner': []}\n\n\n"
    "class Holder:\n    total = 0\n\n\nclass _Box:\n    value = 0\n\n\nBOX = _Box()\n"
)


def state_rule(*, code: str, body: str, header: str = "", function: str = "check") -> str:
    """Return one model-subject custom Rule module whose check runs `body` and finds nothing."""

    indented: str = "".join(f"    {line}\n" for line in body.splitlines())
    return (
        f"{STATE_RULE_HEADER}{header}\n\n"
        f'@rule(code="{code}", message="orders state", remediation="Keep rules pure.")\n'
        f"def {function}(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
        f"{indented}    return []\n"
    )


def order_helpers_file() -> tuple[str, str]:
    """Return the shared helper module with constant tables, a registry, and an lru cache."""

    return ("rules/helpers.py", ORDER_HELPERS)


def stateless_rule_file(code: str = STATELESS_RULE_CODE) -> tuple[str, str]:
    """Return a rule that only reads the shared helper tables."""

    return (
        f"rules/{code.lower()}.py",
        state_rule(
            code=code,
            header=IMPORT_ORDER_HELPERS + "LIMITS = {'orders': (1, 2, 3)}\n",
            body="_ = helpers.is_known(model.name) and model.name in helpers.REGION_CODES",
        ),
    )


def state_case_files(*, header: str, body: str) -> tuple[tuple[str, str], ...]:
    """Return the helper, one rule running `body` under `header`, and one stateless rule."""

    return (
        order_helpers_file(),
        ("rules/orders_state.py", state_rule(code=STATEFUL_RULE_CODE, header=header, body=body)),
        stateless_rule_file(),
    )


def random_state_files(*, seed: int, model_count: int) -> tuple[tuple[str, str], ...]:
    """Return two to five rules each running one seeded mutation for a seeded set of models."""

    generator: random.Random = random.Random(seed)
    files: list[tuple[str, str]] = [order_helpers_file()]
    for index in range(generator.randint(2, 5)):
        operation: str = generator.choice(RANDOM_OPERATIONS)
        triggers: list[int] = sorted(generator.sample(range(model_count), generator.randint(0, 3)))
        indented: str = "".join(f"    {line}\n" for line in operation.splitlines())
        files.append(
            (
                f"rules/random_{index}.py",
                state_rule(
                    code=f"XSQBRSR{index:03d}",
                    header=generator.choice(RANDOM_HEADERS) + RANDOM_STATE,
                    body=f"if model.index in {triggers!r}:\n{indented}",
                ),
            )
        )
    return tuple(files)


class StateDetector(Protocol):
    """A module-state detector reporting newly stateful codes after each call."""

    def changed_codes(self) -> tuple[str, ...]: ...


class ReferenceStateDetector:
    """The previous detector: re-fingerprint every remaining code after each call."""

    def __init__(
        self, *, namespaces: dict[str, tuple[dict[str, object], ...]], rules_root: Path
    ) -> None:
        self._namespaces: dict[str, tuple[dict[str, object], ...]] = namespaces
        self._rules_root: Path = rules_root
        self._initial: dict[str, str | None] = {code: self._token(code) for code in namespaces}
        self._pending: tuple[str, ...] = tuple(namespaces)

    def changed_codes(self) -> tuple[str, ...]:
        tokens: dict[str, str | None] = {code: self._token(code) for code in self._pending}
        changed: tuple[str, ...] = tuple(
            filter(
                lambda code: tokens[code] is None or tokens[code] != self._initial[code],
                self._pending,
            )
        )
        self._pending = tuple(filter(lambda code: code not in changed, self._pending))
        return changed

    def _token(self, code: str) -> str | None:
        try:
            return module_state_token(
                namespaces=self._namespaces[code], rules_root=self._rules_root
            )
        except OpaqueModuleStateError:
            return None


def load_state_rules(*, root: Path, files: tuple[tuple[str, str], ...]) -> tuple[Rule, ...]:
    """Write custom-rule files beneath `root` and load every custom rule they define."""

    # A packaged `rules` unloads with the project, so no later test imports a stale helper module.
    for relative_path, contents in (("rules/__init__.py", ""), *files):
        path: Path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
    return tuple(
        filter(
            attrgetter("custom"),
            build_rule_catalogue(config=RulesConfig(), project_dir=root, include_custom=True),
        )
    )


def _rule_namespaces(rules: tuple[Rule, ...]) -> dict[str, tuple[dict[str, object], ...]]:
    return {rule.code: rule_namespaces((rule.check,)) for rule in rules}


def reference_detector(*, rules: tuple[Rule, ...], root: Path) -> StateDetector:
    """Return the full re-fingerprinting reference detector for `rules`."""

    return ReferenceStateDetector(namespaces=_rule_namespaces(rules), rules_root=root / "rules")


def monitor_detector(*, rules: tuple[Rule, ...], root: Path) -> StateDetector:
    """Return the production module-state monitor for `rules`."""

    return ModuleStateMonitor(namespaces=_rule_namespaces(rules), rules_root=root / "rules")


def state_histories(
    *, rules: tuple[Rule, ...], model_count: int, detectors: tuple[StateDetector, ...]
) -> tuple[list[tuple[str, ...]], ...]:
    """Run every rule over numbered order models and record each detector after each call."""

    histories: tuple[list[tuple[str, ...]], ...] = tuple([] for _ in detectors)
    for index in range(model_count):
        subject: SimpleNamespace = SimpleNamespace(name=f"orders_{index:03d}", index=index)
        for rule in rules:
            _ = rule.check(
                **{str(rule.subject_parameter): subject, str(rule.context_parameter): None}
            )
            for history, detector in zip(histories, detectors, strict=True):
                history.append(tuple(sorted(detector.changed_codes())))
    return histories


def compare_state_detectors(
    *, root: Path, files: tuple[tuple[str, str], ...], model_count: int
) -> tuple[list[tuple[str, ...]], list[tuple[str, ...]]]:
    """Return the reference and monitor histories observed side by side over the same calls."""

    rules: tuple[Rule, ...] = load_state_rules(root=root, files=files)
    reference, monitor = state_histories(
        rules=rules,
        model_count=model_count,
        detectors=(
            reference_detector(rules=rules, root=root),
            monitor_detector(rules=rules, root=root),
        ),
    )
    return reference, monitor


def timed_state_history(
    *,
    root: Path,
    files: tuple[tuple[str, str], ...],
    model_count: int,
    detector: Callable[..., StateDetector],
) -> tuple[float, list[tuple[str, ...]]]:
    """Return the CPU seconds one detector spends over every call, and its history."""

    rules: tuple[Rule, ...] = load_state_rules(root=root, files=files)
    started: float = time.process_time()
    (history,) = state_histories(
        rules=rules, model_count=model_count, detectors=(detector(rules=rules, root=root),)
    )
    return time.process_time() - started, history


def touch_files(*, root: Path, names: tuple[str, ...]) -> None:
    """Create empty files beneath one folder."""

    for name in names:
        (root / name).touch()

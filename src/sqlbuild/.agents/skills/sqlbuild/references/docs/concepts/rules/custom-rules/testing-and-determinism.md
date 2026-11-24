<!-- generated-by: sqlbuild skills -->

# Testing and deterministic Rules

> Test custom findings and keep implementations safe for dependency-aware caching.

Online: https://sqlbuild.com/docs/concepts/rules/custom-rules/testing-and-determinism/

## Contents

- Test Rules
- Parallel evaluation
- Helpers
- Deterministic inputs
- Cache granularity

## Test Rules

Use the public harness to exercise discovery, compilation, and Rules evaluation:

```python
from sqlbuild.rules.testing import RuleCase, evaluate_rule
from rules.final_outputs import final_order_identifier

def test_given_missing_order_id_when_evaluating_then_reports_finding() -> None:
    result = evaluate_rule(
        rule=final_order_identifier,
        test_case=RuleCase(
            description="missing order identifier",
            source='MODEL (description "Orders");\nSELECT 1 AS customer_id\n',
            path="sql/models/final/customer_orders.sql",
            expected_finding_count=1,
        ),
    )

    assert result.finding_count == 1
```

`RuleCase.files` adds supporting project files. `RuleCase.config` supplies option values. Include
passing cases, failing cases, near misses, and exact source-position assertions.

When the Rule's file lives in the project's `rules/` directory, the harness loads it with that whole
directory available, as `sqb compile` does, so a Rule that imports a shared helper such as
`from rules._helpers.naming import has_layer_prefix` runs under test too.

The test imports the Rule from the `rules` package, so the project directory must be on the Python
module search path. Under pytest's default import mode, a `conftest.py` in the project directory
adds it. With `--import-mode=importlib`, pytest adds no test directories, so set `pythonpath`
instead:

```toml
# pyproject.toml
[tool.pytest.ini_options]
pythonpath = ["."]
```

Each case also checks determinism. The harness evaluates every subject once, then evaluates every
subject again in reverse order in the same process. If the Rule's findings differ between the two
passes, the case fails with `custom rule <code> is not deterministic`. The usual cause is
module-level state, such as a set of already-seen models, that carries over from one subject to
the next. Compute findings from the subject and `ctx` alone.

Keep Rule tests under the project's `tests/` directory, outside `tests/python/nodes/`, `tests/python/providers/`
and `tests/orchestration/`, such as in `tests/rules/`. `sqb test` runs only those three folders, so
run Rule tests with pytest.

## Parallel evaluation

On large projects, SQLBuild splits custom Rule evaluation across several host processes, at most
one per available CPU and never more than eight. Each process evaluates a share of the subjects,
and SQLBuild merges the findings back into the order a single process reports them.

A Rule that changes module-level state while it runs, for example by adding each model to a set of
already-seen names, could report different findings when its subjects are split. SQLBuild detects
the state change and evaluates that Rule again in one process over all of its subjects, so its
findings and cache entries match a single-process run. The result stays correct, but the Rule no
longer runs in parallel and costs an extra host process. Keep each subject's findings independent
of the others so Rules can run in parallel.

SQLBuild compares module state once per model, after every selected Rule has evaluated it, and
after each project-subject Rule invocation. A change still present then is reported against each
Rule whose module, or helper module it imports, holds the changed value, the same Rules a check after
every invocation would name. All of one model's Rules run in the same process, so a change undone
before the model's last Rule finishes, such as a scratch list one Rule fills and another empties for
the same model, is not detected and cannot change findings. Project Rules can run in different
processes, so they are still checked after each invocation. The same check decides which Rules the cache can track by the
facts they read: a Rule whose module state changed is cached against every project fact instead, so
any edit evaluates it again. `compile_timings` reports the time this check takes as
`custom_rules_monitor_cpu_ms`; see [`sqb compile`](../../../cli/compile.md#json-output).

## Helpers

Custom Rules can import repository-owned helpers under `rules/` and these deterministic standard
library modules:

`abc`, `bisect`, `collections`, `copy`, `dataclasses`, `decimal`, `difflib`, `enum`, `fnmatch`,
`fractions`, `functools`, `graphlib`, `hashlib`, `heapq`, `itertools`, `json`, `math`,
`operator`, `pathlib`, `re`, `statistics`, `string`, `textwrap`, `types`, `typing`, and
`unicodedata`, plus `__future__` and `sqlbuild.rules`.

Modules that expose the clock, randomness, the process, or the network are rejected, including
`time`, `datetime`, `random`, `uuid`, `secrets`, `os`, `sys`, `subprocess`, `socket`, and
`importlib`. SQLBuild checks the `import` statements in each selected Rule's file and in every
helper under `rules/` it imports, directly or through other helpers, with absolute or relative
imports anywhere in the module. These are the files the Rules cache fingerprints. Other Python files under
`rules/`, such as tests or scratch scripts, are not checked.

Changing an imported helper invalidates the Rules that depend on it.

Only decorated functions register. Ordinary functions, constants, dataclasses, and classes remain
helpers.

## Deterministic inputs

The Rules cache can only reuse a result when the Rule depends on tracked inputs. SQLBuild runs
custom Rules in a separate host process and enforces that at runtime:

- Rules run with an empty environment, so `os.environ` is empty even when it is reached through
  an allowed module, for example `pathlib.os.environ`.
- The host starts with `PYTHONHASHSEED=0`, so iterating over a set of strings gives the same order
  on every run.
- The working directory is the resolved project directory, wherever `sqb` was invoked, so
  `Path.cwd()` and relative paths do not vary between runs.
- While Rule code runs, opening files, reading file metadata such as `Path.exists()` or
  `Path.stat()`, listing directories, running commands, forking, network access, and `ctypes` are
  rejected, however they are reached. Loading a module outside the
  allowed list for the first time is rejected too.

A rejected operation fails the command with `non-hermetic custom rule <code> at <file>:<line>`, even
if the Rule catches the exception.

The guard exists for cache correctness: it stops Rules from depending on inputs the cache cannot
track by accident. It is not a security sandbox. Deliberately evasive code can bypass it, so only
run Rules you trust.

Read supported project text and structure through `ctx.project.tree`:

```python
text = ctx.project.tree.read_text("rules/requirements.yaml")
matches = ctx.project.tree.glob("sql/models/*/interface/*.sql")
```

Both positive and negative observations are tracked. If `glob` returns no paths, adding a matching
path invalidates the cached result.

## Cache granularity

Cache identity incorporates the whole Rule source file, imported helper closure, configured
options, subject, accessed compiler facts, tracked project observations, dialect, and
compatibility versions. Editing any statement in a Rule file invalidates cached results for every
Rule defined in that file, so keep unrelated Rules in separate files. Prefer model subjects for independent per-model checks and project subjects for genuine
cross-project invariants.

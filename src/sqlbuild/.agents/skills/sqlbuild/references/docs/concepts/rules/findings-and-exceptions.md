<!-- generated-by: sqlbuild skills -->

# Findings and exceptions

> Understand Rule diagnostics and record intentional exceptions safely.

Online: https://sqlbuild.com/docs/concepts/rules/findings-and-exceptions/

## Contents

- Exact exceptions
- Relation-level exceptions
- Path- and resource-scoped ignores
- No inline suppressions
- Removed settings
- Mandatory correctness
- Machine-readable findings

A Rule finding identifies an exact code, project-relative path, line, column, message, and
remediation. Error findings make compilation fail and prevent artifact completion.

Every deliberate deviation from a selected Rule lives in `sqlbuild_project.toml`, never in SQL
comments. A change to the project file is something a reviewer stops to look at; a comment in a
large SQL diff is not.

## Exact exceptions

Use an exact exception for one known finding. Exceptions require a reason and are stale-checked:

```toml
[[rules.rule_exceptions]]
rule = "SQLCLEAN004"
path = "models/examples/sample_orders.sql"
reason = "This example intentionally demonstrates one sampled row."
```

An exception refers to one exact Rule code and path. If the finding disappears, SQLBuild reports
the stale exception so obsolete configuration does not accumulate silently.

A reason is prose. One written as a multi-line TOML string (`"""..."""`) is stored folded, like a
description: line breaks and the spaces around them become one space, and a blank line starts a
paragraph (see [Folded descriptions](../models/configuration.md#folded-descriptions)).

## Relation-level exceptions

A file-wide exception for a deliberate cartesian product would also hide any accidental cross join
added to the same file later. `SQLCLEAN038` findings name the joined relation and, inside a CTE,
that CTE, so an exception can cover exactly one join:

```toml
[[rules.rule_exceptions]]
rule = "SQLCLEAN038"
path = "models/orders/order_checks.sql"
relation = "order_policy"
cte = "policy_checks"   # optional: only the join inside this top-level CTE
reason = "Every order is evaluated against every policy check."
```

`relation` is the joined relation as written, with `__ref`, `__seed` and `__source` calls resolved
to the referenced name. Without `cte`, the exception covers that relation anywhere in the file.
Relation-level exceptions are stale-checked like exact exceptions: an entry that matches no finding
fails compilation. `relation` is accepted only for `SQLCLEAN038`, and a file-wide exception cannot be
combined with relation-level exceptions for the same code and path.

A join that reads no named relation, such as `CROSS JOIN UNNEST(...)` or `CROSS JOIN LATERAL`, has
no relation to name. Its finding says so and prints the file-wide exception (rule and path) that is
the only way to cover it.

`SQLCLEAN038` does not report a cross join when either side provably holds at most one row, so most
intentional cross joins need no exception at all. SQLBuild infers this from the query, never from a
declaration: a seed whose CSV has one data row (adding a second row brings the finding back), an
aggregate without `GROUP BY`, a one-row `VALUES`, `LIMIT 1`, or `QUALIFY ROW_NUMBER() OVER (...) = 1`
without a partition, carried through CTEs and subqueries that only filter or project. A join,
`UNNEST`/`FLATTEN`, set operation or `GROUP BY` in between stops the proof, and the join is reported.

## Path- and resource-scoped ignores

Use a path ignore when a documented project area intentionally follows a different convention:

```toml
[[rules.rule_ignores]]
rules = ["SQLCLEAN"]
paths = ["models/examples/**"]
reason = "Examples retain intentionally minimal SQL."
```

Path ignores accept exact codes and family prefixes. Keep their scope narrow and explain why the
project differs from the selected requirement. A path without glob characters must exist, so an
ignore left behind by a renamed or deleted file fails compilation with
`rule ignore path does not exist: <path>`; move the ignore to the new path or delete it.

Use `selectors` when the exception follows graph-resource identities or lineage rather than files:

```toml
[[rules.rule_ignores]]
rules = ["SQLCLEAN021"]
selectors = ["intermediate_*"]
reason = "These intermediate interfaces intentionally preserve upstream columns."
```

Selectors use the same grammar as SQLBuild commands, including exact names, name globs, tags,
resource paths, and graph expansion such as `+intermediate_*`. Use `paths` for authored-file glob
matching, including SQL tests and audits that are not graph resources:

```toml
[[rules.rule_ignores]]
rules = ["SQLCLEAN021"]
paths = ["models/**/intermediate_*.sql"]
reason = "These intermediate SQL files intentionally preserve upstream columns."
```

`paths` and `selectors` may be combined in one scoped ignore. Both forms require a reason.

To turn a Rule off for the whole project, add its code to `[rules] ignore`.

## No inline suppressions

SQLBuild has no inline suppression comment. An `-- sqb: ignore ...` or `/* sqb: ignore ... */`
comment is a `SQBRSUPPRESSION000` error that suppresses nothing, so the finding it was meant to hide is reported too. The error prints the
exact entry to add instead:

```text
error[SQBRSUPPRESSION000]: Inline Rule suppressions are not supported; declare the exception in sqlbuild_project.toml
  --> models/orders/order_checks.sql:4:1
  = help: Remove this comment, then fix the SQLCLEAN038 finding, or record it as a reviewed exception in sqlbuild_project.toml:
            [[rules.rule_exceptions]]
            rule = "SQLCLEAN038"
            path = "models/orders/order_checks.sql"
            relation = "order_policy"
            reason = "every order meets every policy"
```

Copy the entry into `sqlbuild_project.toml`, adjust the reason for a reviewer, and delete the
comment.

## Removed settings

`[rules] allow_exceptions` was removed. Configured exceptions are always allowed because the project
file is the review gate, and inline suppressions no longer exist. A project that still sets
`allow_exceptions` fails configuration loading with an error that explains the removal: delete the
line, keep reviewed deviations as `[[rules.rule_exceptions]]` entries, and use `[rules] ignore` to
turn a Rule off for the whole project.

## Mandatory correctness

Mandatory compiler correctness is not configurable and cannot be suppressed. A project must first
compile into a trustworthy representation before any selected Rule can run.

## Machine-readable findings

Use JSON when another tool consumes findings:

```bash
sqb compile --json
sqb rules --json run SQLCLEAN
```

Machine-readable output remains on stdout. Lifecycle progress and terminal status are written to
stderr.

<!-- generated-by: sqlbuild skills -->

# sqb format

> Apply canonical SQLBuild SQL formatting.

Online: https://sqlbuild.com/docs/cli/format/

## Contents

- Skipping unchanged files
- Line width and descriptions
- What formatting preserves
- Rule fixes
- Unsafe bodies

Formats SQL files using SQLBuild's canonical, dialect-aware representation.

```bash
sqb format [flags]
```

| Flag | Description |
|------|-------------|
| `--check` | Fail when formatting changes are needed without writing |
| `--diff` | Print the proposed diff without writing |
| `--fix` | Also apply compiler-verified, meaning-preserving Rule fixes before layout formatting |
| `--json` | Print structured results |
| `--no-cache` | Format every file instead of skipping files a previous run found clean |
| `--select`, `-s` | Format selected models |
| `--select-file` | Read selectors from a file |
| `--exclude` | Exclude models from the selected scope |

```bash
sqb format
sqb format --check
sqb format --diff --select tag:marts
```

Formatting owns presentation and source rewriting. Compiler-integrated diagnostics belong to
configured [Rules](../concepts/rules.md), which never mutate source.

Files are formatted in parallel; the report, the diff and the exit code are the same as a
one-file-at-a-time run.

## Skipping unchanged files

`sqb format` skips files it already found formatted and clean, as long as the file and the format
settings haven't changed, so `sqb format --check` on an unchanged project is fast. A file that
needed changes or reported anything is always checked again. `--fix` never skips files. Pass
`--no-cache` to format every file.

## Line width and descriptions

Configure the maximum physical line width in `sqlbuild_project.toml` (the default is `100`):

```toml
[format]
line_width = 100
```

`MODEL`, `SCENARIO`, `FUNCTION` and `HOOK` headers need a non-empty `description`. When one is
missing, `sqb format --check` fails with `description-present`, so pre-commit catches it before
compile does. A model described by its `model_schema` or its `[path_defaults]` entry passes, as it
does in compile; see [Descriptions](../concepts/models/configuration.md#descriptions). Plain
`sqb format` still formats the file and exits non-zero.

Descriptions of `MODEL`, `SCENARIO`, `SCHEMA`, `FUNCTION`, `HOOK` and `MACRO` headers have one
canonical layout, which never changes their words, their quotes or the text they store (see
[Folded descriptions](../concepts/models/configuration.md#folded-descriptions)):

- A description that fits on its line stays one line: `"..."`, or `"""..."""` when it contains a
  double quote. A short description wrapped over several lines is joined onto one.
- A longer description, or one with paragraphs, becomes a `"""` block: its lines are indented one
  level under the field and wrapped at the line width, with a blank line between paragraphs, and
  the closing `"""` lines up with the field.

```sql
MODEL (
  description """
    True when a name starts with a woman's title: Mme, Mlle, Miss, Mrs, Ms. Any case, with or
    without a dot ("MISS ANNA PARKER", "Mrs. C. O'Brien", "Ms.Robinson").
  """,
);
```

A one-line `'...'` description that fits is left as written. Running `sqb format` again changes
nothing.

Clause layout follows a few fixed rules:

- A `SELECT` list with exactly one item stays on the `SELECT` line when the item fits (`SELECT *`,
  `SELECT DISTINCT order_id`). Two or more items go one per line.
- `WHERE`, `QUALIFY` and `HAVING` with one top-level condition stay on the clause line when it
  fits (`WHERE amount > 0`). Several `AND`/`OR` conditions go one per line.
- Each CTE closes with `),` followed by one blank line, and one blank line separates the last CTE
  from the statement that uses it. Comments above a CTE stay directly above its name.
- A `VALUES` list that does not fit on its line puts each row on its own line under `VALUES`, and a
  row is never split, even when it is longer than the width.
- When an operand of `+`, `-`, `*`, `/`, `%`, `||`, `AND` or `OR` spans several lines, the
  operator starts the next line.
- A broken `DECODE` puts its first argument on its own line, then one search/result pair per line,
  then any default.

A line longer than the width is broken at its outermost structure first: before top-level `AND`
and `OR`, then before arithmetic operators, then one argument per line in the widest function call,
`IN` list or array, or one `CASE` branch per line. String literals, comments, type parameters such as
`NUMBER(38, 2)`, and lines that carry a comment are never broken, so a line can stay longer than the
width. `sqb format --check` fails while a long line can still be shortened.

## What formatting preserves

Formatting changes layout only: it prints the authored SQL tokens in their original order and
changes only the whitespace between them and the letter case of keywords and built-in function
names. A word is upper-cased only when the parse confirms it is a keyword at that position, such as
`ORDER BY`, `CASE WHEN`, `NULLS LAST`, `IS NULL` or `JOIN`, or when it names a built-in function at
a call site, such as `coalesce(`. Identifiers keep their authored case even when they are spelled
like keywords, such as `t.view`, `AS index` or a table named `view`, and so do aliases, qualified
names, JSON path keys and user-defined function names.

Query fingerprints ignore the case of reserved keywords and built-in function names only, so
formatting lowercase SQL may change a model's fingerprint once, when a non-reserved keyword such as
`by` or `nulls` is upper-cased. Formatting already formatted SQL never changes its fingerprint.

Comments in every dialect form are kept, including `//` in Snowflake and `#` in BigQuery, and
adjacent string literals split across lines stay on separate lines. It does not add `AS` to aliases
or remove statement terminators or trailing commas. Every result is checked against the authored
token stream and comment positions, and a body whose tokens would change is reported as
`format-unsafe` instead of being written.

Formatting preserves authored cast types, postfix casts, quoted literals, variant paths, typed
lambda parameters, and supported SQL function spellings while applying canonical layout. SQLBuild
calls keep their authored spelling, including zero-argument cursor and empty-fixture intrinsics.
CTE-producing macros remain authored calls rather than expanded project SQL, with each call on its
own CTE-list line and leading comments attached to the node they describe.

## Rule fixes

`sqb format` on its own never changes what SQL means. `sqb format --fix` also applies Rule edits
that are known to preserve meaning, then formats the layout. `--fix --check` reports the proposed
changes without writing files, and `--fix --diff` prints their diff.

Edits are planned in memory and repeated until no more apply; overlapping edits wait for the next
pass. Every pass recompiles the project and compares it with the original compilation, so a
project that already has compile errors can still be fixed:

- An edited file may lose diagnostics but must not gain any. Diagnostics are compared by code,
  resource and message, not by line.
- Each edited model must keep its output columns, types and nullability, dependencies, and column
  lineage. Lineage that is unavailable both before and after counts as unchanged when the columns
  match.
- No other file's diagnostics or model outputs may change.

A failure is attributed to the edited file that caused it, and that file keeps its original
contents and reports `format-fix-verification-failed` with the reason, while the other files are
still fixed. When a difference cannot be attributed, each file's edits are checked on their own. A
file whose SQL cannot be expanded is skipped with a `format-fix-skipped` warning that gives the
reason, and the run continues. A cycle or the 128-pass limit fails without writing any edit.

| Rule | Decision | Reason |
| --- | --- | --- |
| SQLCLEAN001 NULL comparison | Refused | Replacing `= NULL` with `IS NULL` changes three-valued logic. |
| SQLCLEAN002 implicit cartesian join | Applied | A comma-separated product becomes an explicit `CROSS JOIN`; mixed join precedence is excluded. |
| SQLCLEAN003 conditionless join | Applied in Snowflake | Snowflake defines an unqualified `JOIN` without `ON` or `USING` as a cartesian product. Other dialects are refused because the original syntax may be invalid. |
| SQLCLEAN005 unused CTE | Applied | Unreferenced `SELECT` CTEs contribute no rows. Mutation statements and commented CTEs are excluded. |
| SQLCLEAN006 redundant DISTINCT | Applied | The projected and grouping expressions are proved to coincide. |
| SQLCLEAN008 bare UNION | Applied | `UNION DISTINCT` states the existing duplicate elimination, in dialects that support it. |
| SQLCLEAN013 parenthesized DISTINCT | Refused | Removing parentheses can change operator precedence. |
| SQLCLEAN017 constant row count | Applied | `COUNT(1)` and `COUNT(*)` both count every row. |
| SQLCLEAN023 unused table alias | Refused | Not yet reviewed as meaning-preserving. |
| SQLCLEAN025 implicit inner join | Applied | A bare `JOIN ... ON` is an inner join. |
| SQLCLEAN030 boolean CASE simplification | Refused | The `COALESCE` rewrite is not proved portable to every supported dialect. |
| SQLCLEAN042 unused CTE output | Applied when row-neutral | Only a column reference or deterministic scalar expression is removed, from a CTE without `DISTINCT`, `DISTINCT ON`, `GROUP BY ALL`, `ORDER BY ALL`, an ungrouped aggregate, or positional `GROUP BY`/`ORDER BY`. Other removals could change the rows and are refused with the reason. |

`--fix` also rewrites a header description that escapes double quotes as `\"` or doubles them as
`""`, which descriptions do not support, as a `"""..."""` string holding real quotes, and reports
it as an applied `description-quotes` fix. It rewrites only a description whose sole backslashes
escape quotes, and only when the new string stores exactly that text. A file that still has header
syntax errors once its descriptions are rewritten is left as written, and the fix is reported as
refused. When the project cannot compile
until the rewritten descriptions are saved, the run skips the compiler-verified Rule fixes with a
`format-fix-skipped` warning; run `sqb format --fix` again to apply them.

Edits that touch macro or intrinsic expansions are refused, and so are edits outside model SQL
bodies, because recompiling cannot prove their output contract. Findings without a proposed edit
are reported as unavailable. Human output lists each applied, refused, and unavailable fix with its
reason; JSON output includes a `rule_fixes` array with `file`, `line`, `code`, `status`, and
`reason`. Refused and unavailable fixes still need a manual change.

## Unsafe bodies

`sqb format` reports a file-specific `format-unsafe` fault whenever a SQL body cannot be safely
formatted, including parser, comment-attachment, interpolation-restoration, and idempotence
failures. The entire original file is kept (including its headers and fixtures), the reason
appears in human and JSON output, and both formatting and `sqb format --check` exit nonzero. A
declined body is never counted as canonical.

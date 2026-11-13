<!-- generated-by: sqlbuild skills -->

# Overview

> Limit enums, constants, macros, audits, schemas, and named hooks to the parts of a project that use them.

Online: https://sqlbuild.com/docs/concepts/declaration-scopes/

## Contents

- Example
- Labels in `sqb scope` output
- Choose a location
- Explore the feature

Most projects start with enums, constants, macros, generic audits, reusable schemas, and named
hooks in their ordinary top-level directories. Those declarations are available throughout the
project.

When a declaration is used only within one folder or its child folders, keep it near the SQL that
uses it. SQLBuild uses the declaration directory's location to
decide which files can access it. No TOML configuration is required.

| Where the declaration lives | Who can use it |
|-------------------------------------|--------------------|
| Top-level `macros/`, `constants/`, or `enums/` | The whole project |
| Nested `_sqlbuild/macros/`, `_sqlbuild/constants/`, or `_sqlbuild/enums/` | Files in the owner folder and folders below it |
| Nested `_sqlbuild/_macros/`, `_sqlbuild/_constants/`, or `_sqlbuild/_enums/` | Files directly in the owner folder only |
| An underscored constant or enum inside `MODEL()` | That model and its inline SQL hooks only |

The same three placements apply to every kind of reusable declaration used by name:

| Kind | Project-wide role | Owner and below | Owner folder only |
|------|-------------------|-----------------|-------------------|
| Macros | `macros/python/`, `macros/sql/` | `_sqlbuild/macros/python/`, `_sqlbuild/macros/sql/` | `_sqlbuild/_macros/python/`, `_sqlbuild/_macros/sql/` |
| Enums | `enums/` | `_sqlbuild/enums/` | `_sqlbuild/_enums/` |
| Constants | `constants/` | `_sqlbuild/constants/` | `_sqlbuild/_constants/` |
| Generic audits | `audits/generic/` | `_sqlbuild/audits/generic/` | `_sqlbuild/_audits/generic/` |
| Singular audits | `audits/singular/` | `_sqlbuild/audits/singular/` | Not supported |
| Reusable schemas | `schemas/` | `_sqlbuild/schemas/` | `_sqlbuild/_schemas/` |
| Named SQL hooks | `hooks/sql/` | `_sqlbuild/hooks/sql/` | `_sqlbuild/_hooks/sql/` |
| Python hooks | `hooks/python/` | `_sqlbuild/hooks/python/` | `_sqlbuild/_hooks/python/` |

Macro and Python hook declarations are Python files (`.py`). All other declarations are SQL files
(`.sql`). Singular audits are never used by name, so they have no folder-only role; a scoped
singular audit may reference only resources at or below its owner.

Functions (`functions/sql/`, `functions/python/`) stay project-wide. They are deployed warehouse
objects and graph nodes, so a compile-time folder restriction would not match what the warehouse
allows.

## Example

The annotation beside each directory shows where its contents are available:

```text
models/
├── constants/                 published throughout models/
│   └── warehouse.sql
└── commerce/
    ├── _sqlbuild/
    │   ├── macros/            published throughout commerce/
    │   │   └── currency.py
    │   └── _enums/            exact commerce/ directory only
    │       └── grain.sql
    ├── orders.sql              sees warehouse, currency, and grain
    ├── finance/
    │   ├── _sqlbuild/
    │   │   └── macros/        published throughout finance/
    │   │       └── tax.py
    │   └── revenue.sql         sees warehouse, currency, and tax
    └── fulfillment/
        └── shipments.sql       sees warehouse and currency
```

The folder containing `_sqlbuild/` is the declaration owner. An unprefixed role applies to that
owner and everything below it. An underscored role applies only to files directly in the owner.
Legacy macro, enum, and constant roles directly below an owner remain supported; audits, schemas,
and hooks are scoped only through `_sqlbuild/`.

`_sqlbuild/` is reserved for the declaration-role directories in the table above. Other direct
files or folders are rejected, as are unknown entries inside an `audits/` or `hooks/` role. It must
also sit below a concrete owner directory. A project-root `_sqlbuild/` or authored-root path such as
`models/_sqlbuild/` is invalid; declarations at that boundary belong in the project-wide roots.

A declaration file resolves the macros, enums, and constants it uses from its owner folder, not
from the role directory that contains it. A generic audit in `models/marts/_sqlbuild/_audits/generic/`
therefore sees the same declarations as a model in `models/marts/`. Those uses must be visible from
the owner folder, or compilation fails; they never change where the declaration itself must live.

| Resource | Visible from the example tree | Not visible |
|----------|-------------------------------|-------------|
| `orders.sql` | Warehouse constant, commerce macro, commerce-folder-only enum | Finance macro |
| `finance/revenue.sql` | Warehouse constant, commerce macro, finance macro | Commerce-folder-only enum |
| `fulfillment/shipments.sql` | Warehouse constant, commerce macro | Commerce-folder-only enum, finance macro |

Declarations do not flow upward or sideways into sibling directories.

## Labels in `sqb scope` output

The CLI's text output uses short labels for the same four rules:

| Label | Meaning |
|-----------------|------------------------|
| `project` | Available throughout the project |
| `descendant-public` | Available in this folder and folders below it |
| `exact-owner-private` | Available directly in this folder only |
| `model-private` | Available to one model only |

Here, an **owner folder** is simply the folder directly containing a model, test, hook, function, or
other authored resource. The [Scope Explorer](declaration-scopes/explorer.md) shows the owner
folder, its parents, and project-wide declarations separately.

## Choose a location

| Where the value is needed | Placement |
|---------------------------|-----------|
| One model only | In that model's `MODEL()` header |
| Files directly in one folder | In an underscored role under the owner's `_sqlbuild/` folder |
| Files across one folder tree | In an unprefixed role under the nearest shared owner's `_sqlbuild/` folder |
| Different resource trees, such as models and tests | In a top-level declaration role |

Apply the same rule to generic audits, schemas, and hooks by looking at the models, sources, and
seeds that attach or call them, including attachments generated by audit factories. For a singular
audit, look at the resources it references.

SQLBuild computes the lowest common owner of every declaration's runtime consumers. A project-wide
declaration is valid only when no narrower supported owner contains all consumers. This keeps the
top-level roles as a genuine project API instead of a neutral dumping ground.

## Explore the feature

    See which declarations a model, test, hook, or function can use.
    Choose the narrowest folder that contains every real use.
    Ask SQLBuild what a file can use and preview how moving it would change that answer.

Learn the features themselves in [Enums](enums.md), [Constants](constants.md),
[Macros](macros.md), [Audits](audits.md),
[Reusable column schemas](models/schemas.md), and [Hooks](models/hooks.md). See [Variables and placeholders](interpolation.md) for project,
environment, and runtime context values, which are separate from declarations.

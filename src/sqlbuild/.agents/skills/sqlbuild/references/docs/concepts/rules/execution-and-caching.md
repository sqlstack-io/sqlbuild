<!-- generated-by: sqlbuild skills -->

# Execution and caching

> Understand Rule enforcement order, focused runs, and dependency-aware reuse.

Online: https://sqlbuild.com/docs/concepts/rules/execution-and-caching/

SQLBuild evaluates a project in this order:

1. discovery, expansion, and mandatory compiler correctness
2. selected native built-in Rules
3. selected custom Python Rules
4. artifact completion

Native built-ins always run before custom Rules. Structured compile output reports built-in and
custom durations separately, together with cache hits and misses.

## What a selection checks

`sqb rules run` is intentionally focused. Use it to inspect one exact code or family, but run the
normal compiler before treating the complete project as valid.

With a selection (`sqb rules run --select`, `sqb plan`, `sqb build`), built-in and custom Rules still
evaluate the whole project with every exception, as `sqb compile` does, so a Rule that reads the graph
sees every dependency and dependent. The selection only filters the report to findings on selected
models and on files that are not models, so a selection never reports a finding that `sqb compile`
would not, and it reuses the cache `sqb compile` writes. SQL lint Rules check each selected file on its
own.

## Dependency-aware caching

Rules cache under `target/rules-cache-native-v1`. A cache identity includes the Rule code, implementation and
imported helpers, options, invocation subject, accessed compiler facts, dialect, tracked project
observations, and compatibility versions. The SQLBuild version in that identity also carries a hash of
the native engine's source, so a local development build never reuses results from a different build
of the same version.

Model-subject Rules support fine-grained reuse. Editing one model invalidates affected model
subjects rather than every Rule invocation. Project-subject Rules run once and are appropriate for
genuinely project-wide invariants, with correspondingly broader invalidation.

Positive and negative project-tree observations are dependencies. If a Rule observes that a glob
has no matches, adding a matching file invalidates that result.

`--no-cache` on `sqb compile`, `sqb plan`, and `sqb build` turns the Rules cache off for that
invocation: every built-in and custom Rule evaluates again, and stored results are neither read nor
written. Use it when a Rule finding looks stale.

## Built-in-only execution

When no custom Rule is selected, SQLBuild does not initialize the custom Python host. When every
custom result is cached, the host is not launched for evaluation.

## Agent guidance

Generate project-specific guidance from the configured Rules:

```bash
sqb rules skills
sqb rules skills --check
```

SQLBuild manages `.agents/skills/sqlbuild-rules/SKILL.md`,
`.claude/skills/sqlbuild-rules/SKILL.md`, and `.opencode/skills/sqlbuild-rules/SKILL.md` without
overwriting divergent or unowned content.

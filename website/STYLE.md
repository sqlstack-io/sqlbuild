# Docs style guide

These rules apply to every page under `website/src/content/docs/`. Pages that follow them well:
`concepts/semantic-compilation.mdx`, `cli/review.mdx`, `concepts/audits.mdx`,
`concepts/refactoring.mdx` and `benchmarks.mdx`.

## Write for users

- Document what a user does, sees and must decide: commands, flags, config keys, defaults, limits,
  error codes and messages, output, and the fix for each problem.
- Leave out how SQLBuild works inside: cache files and their formats, batching, hashing, internal
  tables or structures, and timing internals. Include a mechanism only when the user must act on
  it, and then say what to do (for example, which grant to give or which flag forces a fresh run).
- Never document behaviour you haven't checked on the existing page or in the source.

## Structure

- Lead with what the feature does and a short example: a command, a config block or the output a
  user sees.
- The first paragraph says what the user does and what they get, in two sentences. Conditions,
  exceptions and proofs go in a "Details" list at the end.
- Show the normal path first. If a command writes something for the user, such as `sqb rename`
  adding `migrate_from`, show the command before the hand-written form.
- Organise sections around the user's tasks ("Declaring a rename", "Forcing a fresh compile"),
  not around the implementation.
- One topic per section. Put flags, config keys and codes in reference tables near the top or
  bottom, and details that rarely matter in a short "Details" or "Limitations" list at the end.
- Keep headings that other pages link to. Before renaming or removing one, run
  `grep -rn "<page>#" website/src` and update the links.

## Tables

- Use a table for anything that varies by adapter or dialect: support, grants, sources, limits and
  caveats. Don't write a paragraph per adapter.
- Use tables for flags, config keys with defaults, exit codes, diagnostic codes and checks.
- Keep cells short. Put a long explanation in the text below the table.

## Sentences

- Write short, plain sentences in the present tense. One idea per sentence.
- A paragraph states one rule. If a sentence starts with "unless", "only when" or "except", it is a
  separate rule: give it its own bullet or move it to Details.
- Name the tool, not the engine behind it. Write "SQLBuild's SQL analysis", not the parser library.
  Name other vendors only where the comparison is the point of the page.
- Give every rule an example where it matters, such as the SQL that triggers an error and the SQL
  that fixes it.
- Use neutral orders-style examples (orders, customers, products, payments). See `AGENTS.md`.

## No history

- Describe the product as it is. Don't write "now", "no longer", "used to", "earlier releases",
  "as before", "upgrade note" or "previously". History belongs in `CHANGELOG.md`.
- Drop notes about legacy syntax and migrations from old versions once the error message itself
  tells the user what to do.

## Changing a page

- When a feature changes a page, edit the page into shape: put the new fact in the section it
  belongs to, update tables and examples, and remove what is no longer true. Don't append a
  paragraph to the end of a section or page.
- Don't repeat a rule that another page owns. Link to it instead, for example to
  `concepts/testing#time-limits` for unit test timeout precedence.
- After editing a page, regenerate the skill reference copies with `scripts.generate_docs_skill` and
  commit them with the page.

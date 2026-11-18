# UI copy style

Copy in the UI states facts. Explanations go in help tips and the docs. A test
(`src/lib/copy-style/`) checks the rules it can.

## Rules

1. Labels, column headers, keys, tabs and buttons are nouns or short verb phrases
   of 1–4 words. Use sentence case and no final period.
2. Values and cells are facts: a state, a number, a time or a name. Never write a
   sentence about what the system does.
3. Don't use "will", "is recorded", "so that", "this means", "fails if", "after
   writing" or "you" in labels, values or cells.
4. Explanations go in a help tip (`Panel help`, `HelpTip`, a `KeyValues` row's
   `help`, a `title`) or in the docs, never in `note` or a value.
5. A `Panel note` is a short fact (a count, a time, a scope) of at most 6 words.
   It never restates the title.
6. Put one fact per value. Join facts with " · ", at most 3 segments, and keep
   each segment verbless.
7. Status words come from the shared label maps (`lib/build-area/labels.ts`,
   `lib/operate/labels.ts`, `lib/platform/labels.ts`). The scheduler's or CLI's
   own name for a setting goes in the tooltip as "(name)", not in the value.
8. Empty states say what is missing in a few words ("No runs yet."). How-to
   guidance goes to the docs. A second sentence must add a consequence, not
   repeat the first.
9. Permission gaps get a lock icon and "No access (permission.id)", not a
   sentence.
10. Audited reads get a lock icon with the tooltip "Viewing is audited"
    (`Panel audited`), not inline text.
11. Warnings about consequences stay inline and lead with the consequence
    ("Replaces all data."). They are the only inline sentences allowed.
12. Confirmation dialogs for irreversible actions may use full sentences.
13. Write "CLI" for "from the command line" and "Manual" for "started by hand".
14. Never put user data, API field names or code identifiers in prose. Show them
    as `code` values.

Count with `plural`, `noun` and `nodeCount` from `src/lib/format/plural.ts`, not
local helpers. SQL audits are "audits" and Python `@check` nodes are "Python
checks".

## The copy check

`src/lib/copy-style/copy-style.test.ts` runs with the unit tests. It reads
`src/routes` and `src/lib` (not tests, mocks, `lib/api`, `components/ui`) and
checks these copy sites, with each `{…}` or `${…}` counted as one token:

- values of `Record<…, string>` maps in `labels.ts`, `lib/status.ts` and
  `lib/platform/maintenance.ts`, except maps named `…Help`, `…Hint`,
  `…Technical` or `…Tone`
- `Panel` `title` and `note`, `StatusLabel label`, `label:` and `value:`
  literals (key-value rows, columns, filter options, toolbar counts)
- literal `<th>`, `<dt>` and `<dd>` text
- `empty=` attributes and `class="empty"` / `class="msg"` text

Rules it applies:

- length: labels 4 words, values 5, notes 6, an empty state's first sentence 8
- the banned phrases of rule 3, plus "when it", in labels, values and notes
- no `;`, no `: ` followed by a lowercase clause, at most 3 " · " segments
- a note sharing 2 or more content words with its panel title
- an empty state's second sentence without a consequence word (replace,
  delete, lost, can't, irreversible)

### Exceptions

Mark a deliberate exception on its line or the line before:

```svelte
<!-- copy-allow: consequence warning -->
<p class="note">Rebuilds every model from scratch, replacing all data. Audited.</p>
```

In TypeScript use `// copy-allow: <reason>`. The reason is required: a bare
`copy-allow` is itself a finding. The test logs how many sites are allowed.

### Baseline

The check is report-only: it logs every finding and fails only on a finding
missing from `src/lib/copy-style/baseline.txt`, or on a baseline entry that no
longer occurs. The baseline can therefore only shrink. Entries are
`file | kind | rule | text`, so they survive lines moving. The baseline is
empty today; keep it that way by fixing new findings or marking exceptions.

To rewrite the baseline after fixing findings:

```sh
COPY_BASELINE=write npx vitest run src/lib/copy-style
```

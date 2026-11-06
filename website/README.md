# sqlbuild.com

The SQLBuild website: the homepage at `/` and the documentation at `/docs/`, built as one [Astro Starlight](https://starlight.astro.build) site and styled to match the SQLBuild UI.

- `public/index.html` and `public/styles.css`: the homepage, plain static HTML and CSS. Its terminal examples are real `sqb` output from the projects in `examples/`, trimmed to the relevant lines.
- `src/content/docs/docs/`: the documentation pages (MDX), first ported from the Mintlify docs and now edited here. `src/navigation.mjs` holds the sidebar and redirects.
- `src/components/`: the page layout, header and page title, replacing Starlight's defaults. `src/components/mintlify/` has drop-in versions of the Mintlify components the pages use (`Card`, `CardGroup`, `Frame`, `Note`, `Warning`).
- `public/fonts/`: self-hosted fonts, each with its licence.
- `public/brand.css`: shared SQLStack attribution styling for the homepage and docs. The SQLStack symbols in `public/assets/sqlstack*.png` are unchanged copies from `sqlstack-io/sqlstack` at revision `9b37b9e128d36930e1fe0c5880606525100b228b` (`assets/icons/sqlstack.png` and `sqlstack-dark.png`).

Search uses Pagefind, and `llms.txt`, `llms-small.txt` and `llms-full.txt` are generated at build time.

Requires Node 22.12 or newer.

```bash
npm install
npm run dev      # live preview
npm run build    # static site in dist/, including the search index
npm run preview  # serve dist/
```

Search only works on a built site (`npm run build`, then `npm run preview`).

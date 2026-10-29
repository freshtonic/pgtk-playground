# pgtk playground

Type SQL and see what [pgtk](https://github.com/freshtonic/pgtk) says about it before PostgreSQL
runs it. The page is at <https://freshtonic.github.io/pgtk-playground/>.

- **Execute** splits the text into statements with pgtk-sql's recovering parse, and analyses each
  statement with pgtk-analyze against the catalog of the database. An error gets a red squiggle,
  and the statement does not run. When pgtk-analyze accepts every statement,
  [PGlite](https://pglite.dev) (PostgreSQL 18 in WebAssembly) runs them, and the page shows the
  result of the last one. Each column heading shows the type that pgtk-analyze found and the type
  that PostgreSQL gave.
- **Format** pretty prints the SQL with pgtk-sql's formatter, 80 columns wide. Use it after you
  paste SQL. The page formats the starting query and each Random statement itself. A statement
  that holds a comment, and text that pgtk-sql rejects, stay as they are, because the syntax tree
  has no comments. A comment between two statements stays.
- **Random** gives a random statement from pgtk-sql's sampler. It does not know the schema.
- **Reset database** puts back the starting schema and data.
- **AST** shows the syntax tree of each statement. ⌘-click (Ctrl-click) a type name to open its
  rustdoc page on docs.rs. A variant name opens a rustdoc search, because the tree does not know
  the name of its enum. `web/src/docs-index.json` holds the pages; after a pgtk-sql or recursa
  version changes, run `npm run docs-index` again.

The database starts with a small employee schema (`web/src/schema.sql` and `web/src/data.sql`).
DDL is permitted: after a statement that can change the catalog, the page loads the catalog again,
and the statements after it are checked against the new catalog.

## How it works

- `wasm/` is a Rust crate that uses pgtk-sql, pgtk-catalog and pgtk-analyze, with the
  PostgreSQL 18 grammar. `wasm-pack` builds it for the page.
- Until the next releases of pgtk and recursa, `wasm/Cargo.toml` patches the pgtk crates and the
  recursa crates to commits on GitHub. The pgtk commit has the pretty-print layout of pgtk-sql,
  which needs the layout settings of the recursa commit. The pgtk commit also has the Loader that
  reads no clock, because the browser target has none.
- pgtk-catalog's `Loader` loads the catalog from PGlite, at start-up and after DDL: the page runs
  each of the Loader's queries in PGlite and gives the rows to the Loader.
- pgtk-sql gives a syntax error its location but not PostgreSQL's message. For that message, the
  page asks PostgreSQL (`describeQuery`), which parses the statement but never runs it.
- `web/` is a Vite and TypeScript page with CodeMirror 6.

## Build

You need Rust (the toolchain in `rust-toolchain.toml`), `wasm-pack` and Node.

```sh
cd web
npm ci
npm run wasm   # builds wasm/ into web/src/pkg
npm run dev
```

GitHub Actions builds the page on each push to `main` and deploys it to GitHub Pages.

## Licence

MIT

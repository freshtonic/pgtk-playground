# pgtk playground

Type SQL and see what [pgtk](https://github.com/freshtonic/pgtk) says about it before PostgreSQL
runs it. The page is at <https://freshtonic.github.io/pgtk-playground/>.

- **Execute** splits the text into statements with pgtk-sql's recovering parse, and analyses each
  statement with pgtk-analyze against the catalog of the database. An error gets a red squiggle,
  and the statement does not run. When pgtk-analyze accepts every statement,
  [PGlite](https://pglite.dev) (PostgreSQL 18 in WebAssembly) runs them, and the page shows the
  result of the last one. Each column heading shows the type that pgtk-analyze found and the type
  that PostgreSQL gave.
- **Random** gives a random statement from pgtk-sql's sampler. It does not know the schema.
- **Reset data** puts back the sample data.

The database is a small employee schema (`web/src/schema.sql` and `web/src/data.sql`). The schema
is fixed: the page does not run DDL.

## How it works

- `wasm/` is a Rust crate that uses the published pgtk-sql, pgtk-catalog and pgtk-analyze
  crates, with the PostgreSQL 18 grammar. `wasm-pack` builds it for the page.
- pgtk-catalog's `Loader` loads the catalog from PGlite at start-up: the page runs each of the
  Loader's queries in PGlite and gives the rows to the Loader.
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

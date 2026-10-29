// Starts PGlite with the playground schema, runs pgtk-catalog's Loader queries and writes their
// rows as JSON. Usage: node catalog/dump-rows.mjs <rows.json>   (from the repository root)
import { readFileSync, writeFileSync } from "node:fs";
import { PGlite } from "../web/node_modules/@electric-sql/pglite/dist/index.js";
import init, { loaderBootstrapQuery, loaderQueries } from "../web/src/pkg/pgtk_playground_wasm.js";

const root = new URL("..", import.meta.url);
await init({ module_or_path: readFileSync(new URL("web/src/pkg/pgtk_playground_wasm_bg.wasm", root)) });
const db = await PGlite.create();
await db.exec(readFileSync(new URL("web/src/schema.sql", root), "utf8"));

const rowsOf = async (sql) =>
  (await db.query(sql, [], { rowMode: "array" })).rows.map((row) =>
    row.map((v) => (v === null ? null : String(v))),
  );

const bootstrap = await rowsOf(loaderBootstrapQuery());
const queries = [];
for (const sql of loaderQueries(JSON.stringify(bootstrap))) queries.push(await rowsOf(sql));
writeFileSync(process.argv[2], JSON.stringify({ bootstrap, queries }));
await db.close();

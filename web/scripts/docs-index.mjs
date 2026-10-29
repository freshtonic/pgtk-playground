// Writes src/docs-index.json: for each type name in pgtk-sql and recursa, its rustdoc page on
// docs.rs. The versions come from wasm/Cargo.lock. Run it again after a version changes:
//   npm run docs-index
import { readFileSync, writeFileSync } from "node:fs";

const lock = readFileSync(new URL("../../wasm/Cargo.lock", import.meta.url), "utf8");
const version = (name) => {
  const m = new RegExp(`name = "${name}"\\nversion = "([^"]+)"`).exec(lock);
  if (!m) throw new Error(`${name} is not in wasm/Cargo.lock`);
  return m[1];
};

const crates = ["pgtk-sql", "recursa"].map((name) => {
  const base = `https://docs.rs/${name}/${version(name)}/${name.replace(/-/g, "_")}/`;
  return { name, base };
});

// Each path is relative to the base of its crate: "<crate index>:<path>".
const types = {};
for (const [i, { base }] of crates.entries()) {
  const response = await fetch(`${base}all.html`);
  if (!response.ok) throw new Error(`${base}all.html: ${response.status}`);
  const html = await response.text();
  for (const [, path, name] of html.matchAll(/href="((?:[a-z0-9_]+\/)*(?:struct|enum|type|union)\.([A-Za-z0-9_]+)\.html)"/g)) {
    (types[name] ??= []).push(`${i}:${path}`);
  }
}
const index = {
  bases: crates.map((c) => c.base),
  // A rustdoc search, for a variant or for a name that more than one type has.
  search: `${crates[0].base}?search=`,
  types,
};
writeFileSync(new URL("../src/docs-index.json", import.meta.url), JSON.stringify(index));
console.log(`${Object.keys(types).length} type names`);

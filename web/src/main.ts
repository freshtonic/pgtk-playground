import { PGlite, type Results } from "@electric-sql/pglite";
import { EditorView, basicSetup } from "codemirror";
import { keymap } from "@codemirror/view";
import { sql, PostgreSQL } from "@codemirror/lang-sql";
import { linter, setDiagnostics, type Diagnostic } from "@codemirror/lint";
import init, { Playground, ast as astJson } from "./pkg/pgtk_playground_wasm.js";
// The Catalog of this database, from `npm run catalog` (see catalog/ in the repository root).
import catalogJson from "./catalog.json?raw";
import schemaSql from "./schema.sql?raw";
import dataSql from "./data.sql?raw";

// ----- what the wasm check gives (see wasm/src/lib.rs) -----

type TypeName = { oid: number; type: string };
type Item = { start: number; end: number; text: string } & (
  | {
      kind: "rejected";
      atStart: number;
      atEnd: number;
      found: string | null;
      expected: string[];
    }
  | {
      kind: "diagnostic";
      sqlstate: string;
      message: string;
      detail: string | null;
      hint: string | null;
      atStart: number | null;
      atEnd: number | null;
    }
  | { kind: "unsupported"; what: string }
  | {
      kind: "analysed";
      utility: boolean;
      columns: (TypeName & { name: string })[];
      params: TypeName[];
    }
);

/** One line in the notes above the results. A note with a range also gets a squiggle. */
type Note = {
  severity: "error" | "warning" | "info";
  title: string;
  message: string;
  from?: number;
  to?: number;
};

type PgError = Error & { code?: string; position?: string; hint?: string; detail?: string };

const DEFAULT_SQL = `-- The salary bill of each department, with its best-paid employee.
SELECT d.name AS department,
       count(e.id) AS headcount,
       sum(e.salary) AS salary_bill,
       max(e.salary) AS top_salary
FROM departments d
LEFT JOIN employees e ON e.department_id = d.id AND e.active
GROUP BY d.name
ORDER BY salary_bill DESC NULLS LAST;
`;

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const executeButton = $<HTMLButtonElement>("execute");
const randomButton = $<HTMLButtonElement>("random");
const resetButton = $<HTMLButtonElement>("reset");
const status = $<HTMLSpanElement>("status");
const notesEl = $<HTMLDivElement>("notes");
const resultsEl = $<HTMLDivElement>("results");
$<HTMLPreElement>("schema").textContent = schemaSql;

// ----- the editor -----

const editor = new EditorView({
  parent: $("editor"),
  doc: DEFAULT_SQL,
  extensions: [
    basicSetup,
    sql({ dialect: PostgreSQL }),
    // The diagnostics come from Execute, not from a lint source.
    linter(null),
    keymap.of([{ key: "Mod-Enter", run: () => (void busy("Running…", execute), true) }]),
    EditorView.updateListener.of((update) => {
      // Clear the squiggles on an edit.
      if (update.docChanged) update.view.dispatch(setDiagnostics(update.state, []));
    }),
  ],
});

/** Makes a zero-width range cover the token at that location, so that it can be seen. */
function widen(text: string, from: number, to: number): [number, number] {
  if (to > from) return [from, to];
  if (from >= text.length) return [Math.max(0, text.length - 1), text.length];
  const token = /^("(?:[^"]|"")*"|'(?:[^']|'')*'|[\p{L}\p{N}_$.]+|\S)/u.exec(text.slice(from));
  return [from, from + (token ? token[0].length : 1)];
}

function showNotes(notes: Note[]) {
  const text = editor.state.doc.toString();
  const diagnostics: Diagnostic[] = notes
    .filter((n) => n.from !== undefined)
    .map((n) => {
      const [from, to] = widen(text, n.from!, n.to ?? n.from!);
      return { from, to, severity: n.severity, message: `${n.title}: ${n.message}` };
    });
  editor.dispatch(setDiagnostics(editor.state, diagnostics));
  notesEl.replaceChildren(
    ...notes.map((n) => {
      const div = document.createElement("div");
      div.className = `note ${n.severity}`;
      const title = document.createElement("strong");
      title.textContent = n.title;
      const message = document.createElement("span");
      message.textContent = n.message;
      div.append(title, " ", message);
      if (n.from !== undefined) {
        div.classList.add("located");
        div.onclick = () => {
          editor.dispatch({ selection: { anchor: n.from! }, scrollIntoView: true });
          editor.focus();
        };
      }
      return div;
    }),
  );
}

// ----- PostgreSQL and the catalog -----

let db: PGlite;
let playground: Playground;
const typeNames = new Map<number, string>();

async function rowsOf(sqlText: string): Promise<(string | null)[][]> {
  const result = await db.query<unknown[]>(sqlText, [], { rowMode: "array" });
  return result.rows.map((row) => row.map((v) => (v === null ? null : String(v))));
}

/** Puts back the sample data. The tables stay, so that the Catalog stays correct. */
async function resetData() {
  await db.exec("TRUNCATE projects, employees, departments RESTART IDENTITY CASCADE;");
  await db.exec(dataSql);
}

async function start() {
  await init();
  playground = new Playground(catalogJson);
  db = await PGlite.create();
  await db.exec(schemaSql);
  await db.exec(dataSql);
  for (const [oid, name] of await rowsOf("SELECT oid::text, typname::text FROM pg_type")) {
    typeNames.set(Number(oid), name!);
  }
}

// ----- Execute -----

/** True for a statement that changes the schema. The schema is fixed, so the page refuses it. */
function isDdl(text: string): boolean {
  const body = text.replace(/^(\s+|--[^\n]*\n?|\/\*[\s\S]*?\*\/)*/, "");
  return /^(create|alter|drop|truncate|grant|revoke|comment|reindex|cluster|security|import|refresh)\b/i.test(
    body,
  );
}

/** Converts PostgreSQL's 1-based character position in `text` to a UTF-16 offset. */
function pgPosition(text: string, position: string | undefined): number | undefined {
  const n = Number(position);
  if (!Number.isInteger(n) || n < 1) return undefined;
  let offset = 0;
  let chars = 0;
  for (const ch of text) {
    if (++chars >= n) break;
    offset += ch.length;
  }
  return offset;
}

function pgMessage(e: PgError): string {
  return [e.message, e.detail && `Detail: ${e.detail}`, e.hint && `Hint: ${e.hint}`]
    .filter(Boolean)
    .join(" ");
}

async function execute() {
  updateAst();
  const text = editor.state.doc.toString();
  const items: Item[] = JSON.parse(playground.check(text));
  const notes: Note[] = [];
  let blocked = false;

  for (const item of items) {
    if (item.kind === "rejected") {
      blocked = true;
      // The PostgreSQL part of a Rejection comes from PostgreSQL. describeQuery parses and
      // analyses the statement but never runs it.
      try {
        await db.describeQuery(item.text);
        notes.push({
          severity: "warning",
          title: "pgtk-sql rejects this statement, but PostgreSQL accepts it.",
          message: "This is a pgtk bug. Please report it.",
          from: item.atStart,
          to: item.atEnd,
        });
      } catch (e) {
        const err = e as PgError;
        notes.push({
          severity: "error",
          title: `${err.code ?? "ERROR"}`,
          message: `${pgMessage(err)} (found by pgtk-sql; message from PostgreSQL)`,
          from: item.atStart,
          to: item.atEnd,
        });
      }
    } else if (item.kind === "diagnostic") {
      blocked = true;
      notes.push({
        severity: "error",
        title: item.sqlstate,
        message:
          [item.message, item.detail && `Detail: ${item.detail}`, item.hint && `Hint: ${item.hint}`]
            .filter(Boolean)
            .join(" ") + " (found by pgtk-analyze)",
        from: item.atStart ?? item.start,
        to: item.atEnd ?? item.atStart ?? item.start,
      });
    } else if (item.kind === "unsupported") {
      notes.push({
        severity: "info",
        title: "pgtk-analyze cannot analyse this statement yet",
        message: `(${item.what}). PostgreSQL runs it with no check.`,
        from: item.start,
        to: item.start,
      });
    }
    if (isDdl(item.text)) {
      blocked = true;
      notes.push({
        severity: "error",
        title: "The schema is fixed.",
        message: "The playground does not run DDL. INSERT, UPDATE and DELETE are permitted.",
        from: item.start,
        to: item.start,
      });
    } else if (item.kind === "analysed" && item.params.length > 0) {
      blocked = true;
      notes.push({
        severity: "info",
        title: "Not run: the statement has parameters.",
        message: item.params.map((p, i) => `$${i + 1} ${p.type}`).join(", ") + " (types from pgtk-analyze)",
        from: item.start,
        to: item.start,
      });
    }
  }

  if (blocked || items.length === 0) {
    showNotes(notes);
    resultsEl.replaceChildren(
      muted(items.length === 0 ? "There is no statement to run." : "Not run. See the notes above."),
    );
    return;
  }

  let last: { item: Item; result: Results<unknown[]> } | undefined;
  for (const item of items) {
    try {
      last = { item, result: await db.query<unknown[]>(item.text, [], { rowMode: "array" }) };
    } catch (e) {
      const err = e as PgError;
      const at = pgPosition(item.text, err.position);
      notes.push({
        severity: "error",
        title: `${err.code ?? "ERROR"}`,
        message:
          pgMessage(err) +
          // Parse analysis raises the errors of class 42. The other errors come when the
          // statement runs, and pgtk-analyze does not look for them.
          (item.kind === "analysed" && err.code?.startsWith("42")
            ? " (PostgreSQL found an error that pgtk-analyze did not find)"
            : " (from PostgreSQL, when it ran the statement)"),
        from: item.start + (at ?? 0),
        to: item.start + (at ?? 0),
      });
      showNotes(notes);
      resultsEl.replaceChildren(muted("PostgreSQL stopped at the error above."));
      return;
    }
  }
  showNotes(notes);
  renderResult(last!.item, last!.result);
}

function muted(text: string): HTMLElement {
  const p = document.createElement("p");
  p.className = "muted";
  p.textContent = text;
  return p;
}

function renderResult(item: Item, result: Results<unknown[]>) {
  const children: HTMLElement[] = [];
  if (result.fields.length === 0) {
    children.push(muted(`${result.command ?? "OK"}: ${result.affectedRows ?? 0} rows affected.`));
    resultsEl.replaceChildren(...children);
    return;
  }
  const pgtk = item.kind === "analysed" ? item.columns : undefined;
  const mismatches: string[] = [];
  const table = document.createElement("table");
  const head = table.createTHead().insertRow();
  result.fields.forEach((field, i) => {
    const th = document.createElement("th");
    const pgType = typeNames.get(field.dataTypeID) ?? String(field.dataTypeID);
    const ours = pgtk?.[i];
    const agree = ours?.oid === field.dataTypeID;
    if (ours && !agree) mismatches.push(`${field.name}: pgtk ${ours.type}, PostgreSQL ${pgType}`);
    th.innerHTML = `<span class="col"></span><span class="types"></span>`;
    th.querySelector(".col")!.textContent = field.name;
    th.querySelector(".types")!.textContent = ours
      ? agree
        ? `${pgType} ✓`
        : `pgtk ${ours.type} ≠ ${pgType}`
      : pgType;
    th.title = ours
      ? agree
        ? "pgtk-analyze and PostgreSQL agree on this type."
        : "pgtk-analyze and PostgreSQL disagree on this type."
      : "pgtk-analyze did not analyse this statement.";
    if (ours && !agree) th.classList.add("mismatch");
    head.append(th);
  });
  const body = table.createTBody();
  for (const row of result.rows) {
    const tr = body.insertRow();
    for (const value of row) {
      const td = tr.insertCell();
      if (value === null) {
        td.textContent = "NULL";
        td.className = "null";
      } else {
        td.textContent = value instanceof Date ? value.toISOString().slice(0, 10) : String(value);
      }
    }
  }
  if (mismatches.length > 0) {
    const div = document.createElement("div");
    div.className = "note warning";
    div.textContent = `pgtk-analyze and PostgreSQL disagree on a type (${mismatches.join("; ")}). This is a pgtk bug. Please report it.`;
    children.push(div);
  }
  children.push(muted(`${result.rows.length} ${result.rows.length === 1 ? "row" : "rows"}.`), table);
  resultsEl.replaceChildren(...children);
}

// ----- the AST panel -----

type Primitive = { kind: "string" | "bool" | "number" | "other"; value: string | boolean };
type AstNode =
  | { kind: "struct"; name: string; fields: { name: string; type: AstNode }[] }
  | { kind: "enum"; variant: { name: string; type: AstNode | null } }
  | { kind: "option"; value: AstNode | null }
  | { kind: "list"; items: AstNode[] }
  | { kind: "primitive"; value: Primitive };

const astPane = $<HTMLElement>("ast-pane");
const astEl = $<HTMLDivElement>("ast");
const astButton = $<HTMLButtonElement>("toggle-ast");

function span(className: string, text: string): HTMLSpanElement {
  const s = document.createElement("span");
  s.className = className;
  s.textContent = text;
  return s;
}

/** The label of a node, and its children. A variant that holds one value shows as `A › B`. */
function describe(node: AstNode): { label: (Node | string)[]; children: [string, AstNode][] } {
  switch (node.kind) {
    case "struct": {
      // A token is one line: its text and its kind.
      const [text, kind] = node.fields;
      if (node.name === "Token" && text?.name === "text" && kind?.name === "kind") {
        return { label: [...describe(text.type).label, " ", ...describe(kind.type).label], children: [] };
      }
      return { label: [span("type", node.name)], children: node.fields.map((f) => [f.name, f.type]) };
    }
    case "enum": {
      const inner = node.variant.type;
      if (inner === null) return { label: [span("type", node.variant.name)], children: [] };
      const d = describe(inner);
      return { label: [span("type", node.variant.name), " › ", ...d.label], children: d.children };
    }
    case "option":
      return node.value === null
        ? { label: [span("value", "None")], children: [] }
        : describe(node.value);
    case "list":
      return { label: [span("field", `[${node.items.length}]`)], children: node.items.map((n, i) => [String(i), n]) };
    case "primitive": {
      const v = node.value;
      return { label: [span("value", v.kind === "string" ? JSON.stringify(v.value) : String(v.value))], children: [] };
    }
  }
}

function renderAst(name: string | undefined, node: AstNode, depth: number): HTMLElement {
  const { label, children } = describe(node);
  const head = name === undefined ? label : [span("field", `${name}: `), ...label];
  if (children.length === 0) {
    const div = document.createElement("div");
    div.className = "leaf";
    div.append(...head);
    return div;
  }
  const details = document.createElement("details");
  details.open = depth < 4;
  const summary = document.createElement("summary");
  summary.append(...head);
  details.append(summary, ...children.map(([n, c]) => renderAst(n, c, depth + 1)));
  return details;
}

function updateAst() {
  if (astPane.hidden) return;
  const trees: AstNode[] = JSON.parse(astJson(editor.state.doc.toString()));
  astEl.replaceChildren(
    ...(trees.length === 0
      ? [muted("There is no statement that pgtk-sql accepts.")]
      : trees.map((t, i) => renderAst(trees.length > 1 ? `statement ${i + 1}` : undefined, t, 0))),
  );
}

astButton.onclick = () => {
  astPane.hidden = !astPane.hidden;
  astButton.setAttribute("aria-pressed", String(!astPane.hidden));
  document.querySelector("main")!.classList.toggle("with-ast", !astPane.hidden);
  updateAst();
};

// ----- Random -----

let worker: Worker | undefined;

function randomStatement(): Promise<string | null> {
  worker ??= new Worker(new URL("./random-worker.ts", import.meta.url), { type: "module" });
  const w = worker;
  return new Promise((resolve) => {
    w.onmessage = (e) => resolve(e.data as string | null);
    w.onerror = (e) => {
      e.preventDefault();
      // The sampler failed and trapped the worker's wasm instance. Start a new worker.
      w.terminate();
      worker = undefined;
      resolve(null);
    };
    w.postMessage(null);
  });
}

// ----- start -----

async function busy(label: string, work: () => Promise<void>) {
  const buttons = [executeButton, randomButton, resetButton, astButton];
  buttons.forEach((b) => (b.disabled = true));
  status.textContent = label;
  try {
    await work();
    status.textContent = "";
  } catch (e) {
    status.textContent = `Failed: ${(e as Error).message}`;
    console.error(e);
  } finally {
    buttons.forEach((b) => (b.disabled = false));
  }
}

executeButton.onclick = () => busy("Running…", execute);
resetButton.onclick = () =>
  busy("Resetting the data…", async () => {
    await resetData();
    resultsEl.replaceChildren(muted("The data is reset."));
  });
randomButton.onclick = () =>
  busy("Sampling…", async () => {
    const statement = await randomStatement();
    if (statement === null) {
      showNotes([
        { severity: "info", title: "The sampler failed.", message: "Click Random again." },
      ]);
      return;
    }
    editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: statement + ";\n" } });
    showNotes([]);
    updateAst();
    resultsEl.replaceChildren(
      muted("A random statement from pgtk-sql's sampler. It does not know the schema."),
    );
  });

busy("Loading PostgreSQL…", start);

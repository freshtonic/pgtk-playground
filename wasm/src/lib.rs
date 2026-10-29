//! The WebAssembly side of the playground: the catalog loader, the check that Execute runs
//! before PostgreSQL, the AST, Format and Random.
//!
//! Every offset that crosses to JavaScript is in UTF-16 code units, which is what the editor
//! uses. pgtk gives byte offsets.

use pgtk_analyze::{AnalyzeError, Analyzer, Session, Settings, TargetVersion};
use pgtk_catalog::{Catalog, Loader, QueryId, Rows};
use pgtk_sql::document::{RecoveredItem, parse_sql_recovering};
use recursa::{Pretty, PrettyConfig};
use serde::Serialize;
use wasm_bindgen::prelude::*;

mod ast;

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

/// Drives pgtk-catalog's [`Loader`]. JavaScript runs each query in PGlite and feeds the rows.
#[wasm_bindgen]
pub struct CatalogLoader {
    loader: Loader,
    queries: Vec<(QueryId, &'static str)>,
}

#[wasm_bindgen]
impl CatalogLoader {
    /// The first query. Its rows go to [`CatalogLoader::new`].
    #[wasm_bindgen(js_name = bootstrapQuery)]
    pub fn bootstrap_query() -> String {
        Loader::BOOTSTRAP_QUERY.to_owned()
    }

    /// Starts a load from the rows of the bootstrap query, as JSON (`(string | null)[][]`).
    #[wasm_bindgen(constructor)]
    pub fn new(bootstrap_rows: &str) -> Result<CatalogLoader, JsError> {
        let loader = Loader::new(rows(bootstrap_rows)?)?;
        let queries = loader.queries().collect();
        Ok(CatalogLoader { loader, queries })
    }

    /// The queries to run, in order.
    pub fn queries(&self) -> Vec<String> {
        self.queries
            .iter()
            .map(|(_, sql)| (*sql).to_owned())
            .collect()
    }

    /// Gives the rows of query `index` of [`CatalogLoader::queries`], as JSON.
    pub fn feed(&mut self, index: usize, rows_json: &str) -> Result<(), JsError> {
        let (id, _) = self.queries[index];
        self.loader.feed(id, rows(rows_json)?);
        Ok(())
    }

    /// Ends the load.
    pub fn finish(self) -> Result<Playground, JsError> {
        let catalog = self.loader.finish()?.catalog;
        Ok(Playground { catalog })
    }
}

fn rows(json: &str) -> Result<Rows, JsError> {
    serde_json::from_str(json).map_err(|e| JsError::new(&format!("bad rows: {e}")))
}

/// The analyser, with the Catalog of the PGlite database.
#[wasm_bindgen]
pub struct Playground {
    catalog: Catalog,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Item {
    /// pgtk-sql rejects the text of this statement.
    Rejected {
        start: usize,
        end: usize,
        /// The best single location of the Rejection.
        at_start: usize,
        at_end: usize,
        text: String,
    },
    /// pgtk-analyze found the error that PostgreSQL would raise.
    Diagnostic {
        start: usize,
        end: usize,
        text: String,
        sqlstate: &'static str,
        message: String,
        detail: Option<String>,
        hint: Option<String>,
        /// The location of the error, zero-width when pgtk gives only a cursor.
        at_start: Option<usize>,
        at_end: Option<usize>,
    },
    /// pgtk-analyze cannot analyse this statement yet.
    Unsupported {
        start: usize,
        end: usize,
        text: String,
        what: String,
    },
    /// pgtk-analyze accepts the statement.
    Analysed {
        start: usize,
        end: usize,
        text: String,
        utility: bool,
        columns: Vec<ColumnType>,
        params: Vec<TypeName>,
    },
}

#[derive(Serialize)]
struct ColumnType {
    name: String,
    #[serde(flatten)]
    ty: TypeName,
}

#[derive(Serialize)]
struct TypeName {
    oid: u32,
    #[serde(rename = "type")]
    name: String,
}

#[wasm_bindgen]
impl Playground {
    /// Parses `sql` with the recovering parse and analyses each statement. Gives JSON: an array
    /// of items, one for each statement.
    pub fn check(&self, sql: &str) -> Result<String, JsError> {
        let settings = Settings::new(&self.catalog, TargetVersion::Pg18, ["public"])
            .map_err(|e| JsError::new(&e.to_string()))?;
        let analyzer = Analyzer::new(&self.catalog, settings);
        let document = parse_sql_recovering(sql).map_err(|e| JsError::new(&e.to_string()))?;
        let utf16 = Utf16::new(sql);
        let mut items = Vec::new();
        for item in document.items() {
            let range = item.span().range();
            let text = &sql[range.clone()];
            if text.trim().trim_end_matches(';').trim().is_empty() {
                continue;
            }
            let (start, end) = (utf16.at(range.start), utf16.at(range.end));
            let text = text.to_owned();
            items.push(match item {
                RecoveredItem::Rejected(rejected) => {
                    let rejection = rejected.rejection();
                    let at = rejection.span().range();
                    Item::Rejected {
                        start,
                        end,
                        at_start: utf16.at(at.start),
                        at_end: utf16.at(at.end),
                        text,
                    }
                }
                RecoveredItem::Statement { .. } => {
                    let offset = range.start;
                    match analyzer.analyze_sql(&text, &[], &Session::new()) {
                        Ok(analysis) => Item::Analysed {
                            start,
                            end,
                            text,
                            utility: analysis.utility,
                            columns: analysis
                                .result
                                .columns
                                .iter()
                                .map(|c| ColumnType {
                                    name: c.name.clone(),
                                    ty: self.type_name(c.ty),
                                })
                                .collect(),
                            params: analysis
                                .result
                                .params
                                .iter()
                                .map(|&t| self.type_name(t))
                                .collect(),
                        },
                        Err(AnalyzeError::Diagnostic(d)) => Item::Diagnostic {
                            start,
                            end,
                            text,
                            sqlstate: d.sqlstate,
                            message: d.message.clone(),
                            detail: d.detail.clone(),
                            hint: d.hint.clone(),
                            at_start: d.span.as_ref().map(|s| utf16.at(offset + s.start)),
                            at_end: d.span.as_ref().map(|s| utf16.at(offset + s.end)),
                        },
                        Err(AnalyzeError::Unsupported(u)) => Item::Unsupported {
                            start,
                            end,
                            text,
                            what: u.what.clone(),
                        },
                    }
                }
            });
        }
        serde_json::to_string(&items).map_err(|e| JsError::new(&e.to_string()))
    }

    fn type_name(&self, ty: pgtk_analyze::TypeRef) -> TypeName {
        let ty = &self.catalog[ty];
        TypeName {
            oid: ty.oid.0,
            name: ty.name.as_str().to_owned(),
        }
    }
}

/// The AST of each statement that pgtk-sql accepts, as JSON: an array of [`ast::Node`].
#[wasm_bindgen]
pub fn ast(sql: &str) -> Result<String, JsError> {
    let document = parse_sql_recovering(sql).map_err(|e| JsError::new(&e.to_string()))?;
    let trees: Vec<ast::Node> = document
        .statements()
        .into_iter()
        .map(|statement| ast::Node::from_debug(&format!("{statement:#?}")))
        .collect();
    serde_json::to_string(&trees).map_err(|e| JsError::new(&e.to_string()))
}

/// The line width that the page formats SQL to.
const FORMAT_WIDTH: usize = 80;

/// The indentation of one level of formatted SQL.
const FORMAT_INDENT: usize = 2;

/// Formats `sql` with pgtk-sql's pretty printer: each statement that pgtk-sql accepts, with one
/// blank line between two statements.
///
/// The text that pgtk-sql rejects stays as it is. So does a statement that holds a comment,
/// because the syntax tree has no comments. A comment between two statements stays, on its own
/// lines.
#[wasm_bindgen]
pub fn format(sql: &str) -> Result<String, JsError> {
    let document = parse_sql_recovering(sql).map_err(|e| JsError::new(&e.to_string()))?;
    let config = PrettyConfig::new(FORMAT_WIDTH, FORMAT_INDENT);
    let mut parts = Vec::new();
    let mut end = 0;
    for item in document.items() {
        let span = item.span().range();
        let gap = sql[end..span.start].trim();
        if !gap.is_empty() {
            parts.push(gap.to_owned());
        }
        let text = &sql[span.clone()];
        let formatted = match item {
            RecoveredItem::Statement { parsed, .. } if !has_comment(text) => parsed
                .ast()
                .statement
                .as_ref()
                .map(|statement| statement.render_with(config)),
            RecoveredItem::Statement { .. } | RecoveredItem::Rejected(_) => None,
        };
        parts.push(match formatted {
            Some(statement) if text.trim_end().ends_with(';') => statement + ";",
            Some(statement) => statement,
            None => text.trim().to_owned(),
        });
        end = span.end;
    }
    let tail = sql[end..].trim();
    if !tail.is_empty() {
        parts.push(tail.to_owned());
    }
    let mut formatted = String::new();
    for part in parts.iter().filter(|part| !part.is_empty()) {
        if !formatted.is_empty() {
            // A comment stays just above the statement after it.
            formatted.push_str(if formatted.ends_with(';') {
                "\n\n"
            } else {
                "\n"
            });
        }
        formatted.push_str(part);
    }
    formatted.push('\n');
    Ok(formatted)
}

/// Whether `text` holds a comment: text between two tokens that is not whitespace.
fn has_comment(text: &str) -> bool {
    let lexed = pgtk_sql::lex(text);
    let mut end = 0;
    for token in lexed.tokens() {
        let span = token.span().range();
        if !text[end..span.start].trim().is_empty() {
            return true;
        }
        end = span.end;
    }
    !text[end..].trim().is_empty()
}

/// One random statement from pgtk-sql's sampler, made from `bytes` of entropy, formatted as
/// [`format`] formats a statement.
pub fn random_statement(bytes: &[u8]) -> Option<String> {
    let document = pgtk_sql::ast::Statement::explorer().sample(bytes)?.ok()?;
    let config = PrettyConfig::new(FORMAT_WIDTH, FORMAT_INDENT);
    Some(document.pretty(&config)?.rendered)
}

/// One random statement: tries up to `attempts` times with new bytes from `entropy`.
#[wasm_bindgen]
pub fn random(entropy: &[u8], attempts: usize) -> Option<String> {
    entropy
        .chunks(256)
        .take(attempts)
        .find_map(random_statement)
}

/// Maps byte offsets of a text to UTF-16 offsets.
struct Utf16<'a>(&'a str);

impl<'a> Utf16<'a> {
    fn new(text: &'a str) -> Self {
        Self(text)
    }

    fn at(&self, byte: usize) -> usize {
        self.0[..byte.min(self.0.len())].encode_utf16().count()
    }
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn format_pretty_prints_each_statement_and_keeps_the_comments_between_them() {
        let sql = "-- The departments.\nselect d.name, count(e.id) as headcount, sum(e.salary) as salary_bill, max(e.salary) as top from departments d left join employees e on e.department_id = d.id group by d.name;select 1; /* last */";
        assert_eq!(
            format(sql).unwrap(),
            "-- The departments.\n\
             SELECT\n  d.name,\n  count(e.id) AS headcount,\n  sum(e.salary) AS salary_bill,\n  max(e.salary) AS top\n\
             FROM departments d LEFT JOIN employees e ON e.department_id = d.id\n\
             GROUP BY d.name;\n\
             \n\
             SELECT 1;\n\
             \n\
             /* last */\n"
        );
    }

    #[test]
    fn format_keeps_a_statement_with_a_comment_and_text_that_pgtk_sql_rejects() {
        let sql = "select a, -- the key\n b from t;\nselect from where;";
        assert_eq!(
            format(sql).unwrap(),
            "select a, -- the key\n b from t;\n\nselect from where;\n"
        );
    }
}

//! Writes the Catalog of the playground database as JSON.
//!
//! `dump-rows.mjs` runs pgtk-catalog's Loader queries in PGlite and writes their rows. This tool
//! gives the rows to the same Loader and writes the Catalog. The Loader runs here, not in the
//! browser, because `Loader::finish` reads the clock, and `wasm32-unknown-unknown` has none.
//!
//! Usage: `pgtk-playground-catalog <rows.json> <catalog.json>`

use pgtk_catalog::{Loader, Rows};
use serde::Deserialize;

#[derive(Deserialize)]
struct Dump {
    bootstrap: Rows,
    /// The rows of each query of `Loader::queries`, in order.
    queries: Vec<Rows>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        return Err("usage: pgtk-playground-catalog <rows.json> <catalog.json>".into());
    };
    let dump: Dump = serde_json::from_str(&std::fs::read_to_string(input)?)?;
    let mut loader = Loader::new(dump.bootstrap)?;
    let ids: Vec<_> = loader.queries().map(|(id, _)| id).collect();
    if ids.len() != dump.queries.len() {
        return Err(format!(
            "{} queries, but {} sets of rows",
            ids.len(),
            dump.queries.len()
        )
        .into());
    }
    for (id, rows) in ids.into_iter().zip(dump.queries) {
        loader.feed(id, rows);
    }
    let loaded = loader.finish()?;
    for diagnostic in &loaded.diagnostics {
        eprintln!("note: {diagnostic}");
    }
    std::fs::write(output, serde_json::to_string(&loaded.catalog)?)?;
    Ok(())
}

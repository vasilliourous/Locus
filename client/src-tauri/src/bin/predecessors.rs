//! Thin access to the compiled-in identity table from a **host** tool.
//!
//! # Why not `#[path]`-include the real module
//!
//! `chrysalis::update::predecessor` is written for the GUI crate: it is a child
//! of a module tree that pulls in `tauri`, the hub client and the store. A host
//! script that merely needs to *read the table* must not link any of that, and
//! copying the table into a generator would defeat the purpose of having one
//! source of truth.
//!
//! So this is the second, minimal owner of one `include!`d file:
//! `chrysalis/update/identity_table.rs`, which holds nothing but the struct and
//! the `PREDECESSORS` constant. Both this binary and the client compile the same
//! bytes, so they cannot disagree, and a table row added for a future rename is
//! picked up by the generator with no further edit.
//!
//! # What this binary is for
//!
//! `server/scripts/check-consistency.sh` §54 compares the table against
//! `client/src-tauri/packages/windows/migrate-predecessor.nsh`. Reading it
//! through `cargo run` here rather than re-grepping the Rust means the guard
//! sees exactly what the compiler sees.

#![allow(
    dead_code,
    reason = "the shared table carries fields a given consumer does not read; the point is that \
              both consumers compile the same definition"
)]

// The struct AND the table, in one include: the shared file is the whole
// definition, so a row added for a future rename reaches this tool with no
// second edit. Declaring the struct here as well would be a second definition,
// which the compiler catches (E0428) — which is the right failure.
include!("../chrysalis/update/identity_table.rs");

fn main() {
    // One tab-separated row per line, in table order. Deliberately not JSON: this
    // is consumed by a POSIX shell guard, and a format that needs a parser in the
    // guard is a format that will be parsed wrongly.
    for row in PREDECESSORS {
        println!("{}\t{}\t{}", row.app_id, row.product_name, row.executable);
    }
}

#[cfg(test)]
mod tests {
    use super::PREDECESSORS;

    /// The generator is only useful if the table is non-empty and ordered, so both
    /// are asserted where a mistake is cheapest to see.
    #[test]
    fn the_table_is_not_empty_and_names_locus_first() {
        assert!(!PREDECESSORS.is_empty());
        assert_eq!(
            PREDECESSORS[0].product_name, "Locus",
            "the most recent predecessor must come first; §54 compares order as well as content"
        );
    }
}

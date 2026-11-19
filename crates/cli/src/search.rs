// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use serde_json::Value;

/// Result groups of a catalog search, with the kind that the table shows.
const GROUPS: [(&str, &str); 3] = [
    ("songs", "song"),
    ("albums", "album"),
    ("artists", "artist"),
];

/// One line of the search table.
#[derive(Debug, Eq, PartialEq)]
pub struct Row {
    kind: &'static str,
    id: String,
    name: String,
    artist: String,
}

/// Reads the rows from the raw response of the catalog search.
pub fn rows(response: &Value) -> Vec<Row> {
    GROUPS
        .iter()
        .flat_map(|&(group, kind)| {
            let items = response["results"][group]["data"].as_array();
            items.into_iter().flatten().map(move |item| Row {
                kind,
                id: text(&item["id"]),
                name: text(&item["attributes"]["name"]),
                artist: text(&item["attributes"]["artistName"]),
            })
        })
        .collect()
}

/// Prints the rows as a table with aligned columns.
pub fn print_table(rows: &[Row]) {
    print!("{}", format_table(rows));
}

fn format_table(rows: &[Row]) -> String {
    let width = |field: fn(&Row) -> &str, title: &str| {
        rows.iter()
            .map(|row| field(row).chars().count())
            .chain([title.len()])
            .max()
            .unwrap_or(0)
    };
    let kind_width = width(|row| row.kind, "KIND");
    let id_width = width(|row| row.id.as_str(), "ID");
    let name_width = width(|row| row.name.as_str(), "NAME");
    let line = |kind: &str, id: &str, name: &str, artist: &str| {
        format!("{kind:kind_width$}  {id:id_width$}  {name:name_width$}  {artist}\n")
    };

    let mut table = line("KIND", "ID", "NAME", "ARTIST");
    for row in rows {
        table.push_str(&line(row.kind, &row.id, &row.name, &row.artist));
    }
    table
}

/// Returns a string value, or an empty string for any other JSON value.
fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rows_read_every_group_and_tolerate_missing_fields() {
        let response = json!({ "results": {
            "songs": { "data": [{ "id": "1", "attributes": { "name": "Creep", "artistName": "Radiohead" } }] },
            "artists": { "data": [{ "id": "3", "attributes": { "name": "Radiohead" } }] }
        }});
        let rows = rows(&response);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, "song");
        assert_eq!(rows[0].artist, "Radiohead");
        assert_eq!(rows[1].kind, "artist");
        assert_eq!(rows[1].artist, "");
    }

    #[test]
    fn rows_are_empty_without_results() {
        assert_eq!(rows(&json!({})), Vec::new());
    }

    #[test]
    fn table_aligns_the_columns() {
        let rows = vec![Row {
            kind: "song",
            id: "1".to_owned(),
            name: "Creep".to_owned(),
            artist: "Radiohead".to_owned(),
        }];
        let table = format_table(&rows);
        assert_eq!(
            table,
            "KIND  ID  NAME   ARTIST\nsong  1   Creep  Radiohead\n"
        );
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use schemars::schema_for;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::*;

fn protocol_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocol")
}

fn schema_json() -> String {
    let schema = schema_for!(ProtocolSchema);
    let mut text = serde_json::to_string_pretty(&schema).expect("schema is valid JSON");
    text.push('\n');
    text
}

/// Reads all example files as `(file name, JSON value)`, sorted by name.
fn examples() -> Vec<(String, Value)> {
    let mut files: Vec<PathBuf> = fs::read_dir(protocol_dir().join("examples"))
        .expect("examples directory exists")
        .map(|entry| entry.expect("directory entry is readable").path())
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .expect("file has a name")
                .to_string_lossy()
                .into_owned();
            let text = fs::read_to_string(&path).expect("example is readable");
            let value = serde_json::from_str(&text).expect("example is valid JSON");
            (name, value)
        })
        .collect()
}

/// Checks that `value` survives a typed round trip.
fn assert_round_trip<T: DeserializeOwned + Serialize>(name: &str, value: &Value) {
    let typed: T = serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("{name} does not deserialize: {error}"));
    let again = serde_json::to_value(&typed).expect("typed value serializes");
    assert_eq!(&again, value, "{name} changed in the round trip");
}

#[test]
fn schema_is_up_to_date() {
    let path = protocol_dir().join("schema.json");
    let expected = schema_json();
    if std::env::var_os("RINGO_UPDATE_SCHEMA").is_some_and(|value| value == "1") {
        fs::write(&path, expected).expect("schema file is writable");
        return;
    }
    let current = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        current == expected,
        "protocol/schema.json is out of date. Regenerate it with: \
         RINGO_UPDATE_SCHEMA=1 cargo test -p ringo-engine schema_is_up_to_date"
    );
}

#[test]
fn examples_round_trip_through_the_typed_protocol() {
    for (name, value) in examples() {
        match name.split('.').next().expect("split gives one item") {
            "request" => assert_round_trip::<Request>(&name, &value),
            "notification" => assert_round_trip::<Notification>(&name, &value),
            "success_response" => assert_round_trip::<SuccessResponse>(&name, &value),
            "error_response" => assert_round_trip::<ErrorResponse>(&name, &value),
            prefix => panic!("{name}: unknown example prefix {prefix}"),
        }
    }
}

/// Collects the `method` constants of the variants of a schema definition.
fn schema_methods(schema: &Value, definition: &str) -> BTreeSet<String> {
    schema["$defs"][definition]["oneOf"]
        .as_array()
        .unwrap_or_else(|| panic!("{definition} has no oneOf"))
        .iter()
        .map(|variant| {
            variant["properties"]["method"]["const"]
                .as_str()
                .unwrap_or_else(|| panic!("{definition} variant has no method const"))
                .to_owned()
        })
        .collect()
}

#[test]
fn every_method_and_notification_has_an_example() {
    let schema: Value = serde_json::from_str(&schema_json()).expect("schema is valid JSON");
    let names: BTreeSet<String> = examples().into_iter().map(|(name, _)| name).collect();
    let cases = [("ToEngine", "request"), ("FromEngine", "notification")];
    for (definition, prefix) in cases {
        for method in schema_methods(&schema, definition) {
            let file = format!("{prefix}.{method}.json");
            assert!(
                names.contains(&file),
                "missing example protocol/examples/{file}"
            );
        }
    }
}

#[test]
fn error_code_try_from_returns_error_for_unknown_value() {
    assert_eq!(ErrorCode::try_from(-1), Err(UnknownErrorCode(-1)));
}

#[test]
fn error_code_round_trips_every_numeric_value() {
    for code in [
        -32700, -32600, -32601, -32602, -32603, -32001, -32002, -32003, -32004, -32005,
    ] {
        let parsed = ErrorCode::try_from(code).expect("code is known");
        assert_eq!(parsed.code(), code);
    }
}

#[test]
fn request_deserialize_returns_error_for_wrong_jsonrpc_version() {
    let result =
        serde_json::from_str::<Request>(r#"{"jsonrpc":"1.0","id":1,"method":"player.play"}"#);
    assert!(result.is_err());
}

#[test]
fn protocol_version_matches_the_hello_example() {
    let hello = examples()
        .into_iter()
        .find(|(name, _)| name == "notification.engine.hello.json")
        .expect("hello example exists");
    assert_eq!(hello.1["params"]["proto"], PROTOCOL_VERSION);
}

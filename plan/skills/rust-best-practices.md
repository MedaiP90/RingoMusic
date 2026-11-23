---
name: rust-best-practices
description: Rules for writing, reviewing, and refactoring Rust code (crates, modules, ownership, errors, traits, unsafe, tests, tooling). Use this skill every time Claude writes or changes a .rs file, a Cargo.toml, or a Rust workspace, or when the user asks about Rust style, idioms, clippy, rustfmt, error handling, or crate structure. For GTK user interfaces in Rust, also use the gtk-apps skill.
---

# Rust Best Practices

## Scope

This skill gives rules for Rust code. It is based on these sources:

- Google Rust Style Guide: https://google.github.io/styleguide/rustguide.html
- Canonical Rust Best Practices: https://canonical.github.io/rust-best-practices/
- Agung P.P., "Rust Best Practices and Where to Go Next" (Medium): https://agungpp.medium.com/rust-best-practices-and-where-to-go-next-d13195bcdf85
- The official Rust Style Guide and the Rust API Guidelines, which both of the first two sources use as their base.

This skill does not repeat general rules for naming quality, function size, comments, or tests. The `clean-code` skill has those rules. This skill does not repeat SOLID or encapsulation rules. The `oop-principles` skill has those rules. This skill gives only the Rust-specific rules.

Write all code, identifiers, comments, and doc comments in English.

When two sources do not agree, this skill tells you which rule to use. When the project already has a different, consistent convention, use the project convention. New code must look like the code around it.

## 1. Library code and application code

Some rules are different for library code and for application code.

- **Library code** (a crate that other crates use): API design rules are mandatory. Think about what the callers need.
- **Application code** (a binary, or a crate that only one binary uses): API design rules are strong recommendations. Use the simplest code that works. Do not make code generic before a second use case exists.

## 2. Tooling (do this before every commit)

1. Run `cargo fmt`. Do not change the format by hand.
2. Run `cargo clippy --all-targets --all-features -- -D warnings`. Fix all warnings.
3. Run `cargo test --all-features`.
4. Run `cargo doc --no-deps` for library crates. Fix all broken links.

Use this `rustfmt.toml` for new projects. Remove the unstable options if the project uses stable Rust only.

```toml
max_width = 100
hex_literal_case = "Lower"          # unstable
format_macro_matchers = true        # unstable
imports_granularity = "Module"      # unstable
group_imports = "StdExternalCrate"  # unstable
```

Do not use `#[rustfmt::skip]` for personal preference. Use it only for intentional manual alignment (for example, a matrix) or for generated code.

Do not use unstable (nightly) language features in project code.

Put lints in `Cargo.toml`, not in many `#![allow]` attributes:

```toml
[lints.rust]
unsafe_code = "forbid"   # change to "deny" only if the crate needs unsafe
missing_docs = "warn"    # library crates

[lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
```

When you must allow a lint, use `#[expect(lint_name, reason = "...")]` on the smallest item. Do not use `#[allow]` without a reason.

## 3. Crate and module structure

### 3.1 Crates

- Prefer small crates with one clear purpose. Before you add a large submodule tree, think about a new crate in the workspace.
- Use a Cargo workspace for a project with more than one crate. Put shared dependency versions in `[workspace.dependencies]`.

### 3.2 Modules and files

- Give a file and a leaf module the name of its primary item. The type `Foo` goes in `foo.rs` as `mod foo`.
- Use non-leaf modules only to declare structure. A non-leaf module contains `mod` declarations and re-exports. It does not contain API definitions.
- Use the file layout that the project already uses. For a new project, use `foo.rs` next to a `foo/` directory (the Rust 2018 layout). Do not mix the two layouts in one crate.
- Order the declarations in `lib.rs`, `main.rs`, and module roots in these blocks:
  1. `pub mod`
  2. `pub(crate) mod`
  3. `mod`
  4. `pub use`
  5. `pub(crate) use`
  6. Items with `#[cfg(...)]`, in the same order, one block for each condition.

### 3.3 Error and Result location

- In a library crate, define `Error` in `lib.rs`, below the `mod` and `use` blocks. Define `pub type Result<T, E = Error> = std::result::Result<T, E>;` immediately below it.
- In a binary crate, define `Error` in `src/error.rs`.

## 4. Imports

- Do not use glob imports (`use foo::*`). These are the only exceptions:
  - `use super::*;` in a `#[cfg(test)] mod tests` block.
  - `pub use submodule::*;` in a non-leaf module that re-exports a leaf.
  - The documented prelude of a third-party crate, when the crate requires it (for example, `gtk::prelude::*`).
- Do not glob-import enum variants. If the enum name is long, make a short local alias: `use VeryLongEnumName as E;`.
- Put imports in three blocks, in this order, with one blank line between them:
  1. `std`, `core`, `alloc`
  2. Third-party crates
  3. `crate`, `super`, `self`
- Use one `use` statement for each module. Do not put `::` inside braces.

```rust
// Correct
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read};

// Incorrect
use std::{collections::{BTreeMap, HashMap}, io::{self, Read}};
```

- Import types and traits directly. Use the type name without the module path.
- Call free functions with their module name: `mem::swap(a, b)`, `fs::read_to_string(path)`.
- If the method calls of a trait occur in only one function, import the trait in that function.
- Use the module path when a short type name is ambiguous: `io::Error`, `fmt::Result`.

## 5. Naming

Follow RFC 430 case rules:

| Item | Case |
|---|---|
| Crates, modules, functions, methods, variables | `snake_case` |
| Types, traits, enum variants | `UpperCamelCase` |
| Constants, statics | `SCREAMING_SNAKE_CASE` |
| Generic type parameters | One uppercase letter (`T`, `K`, `V`) |
| Lifetimes | A short lowercase word (`'src`, `'buf`) |

Specific rules:

- **Acronyms**: Write acronyms as one word: `HttpClient`, `IpAddr`, not `HTTPClient`.
- **Abbreviations**: Do not use abbreviations that a new reader does not know. Do not remove letters from words (`msg_cnt` is incorrect; `message_count` is correct).
- **Conversion methods**: Use the standard prefixes with the correct `self` type:
  - `as_foo(&self) -> &Foo`: free, borrowed view.
  - `to_foo(&self) -> Foo`: costs work, gives an owned value.
  - `into_foo(self) -> Foo`: consumes `self`.
- **Constructors**: Use `new()`. Use `new_with_foo()` or `from_foo()` for other constructors. Use `try_new()` for a constructor that can fail.
- **Getters and setters**:
  - `foo()` returns `&T`, or `T` if `T` is `Copy`. Do not use a `get_` prefix.
  - `foo_mut()` returns `&mut T`.
  - `set_foo(value: T)` takes the value.
  - `take_foo()` clears the field and returns the old value.
  - `replace_foo(value: T)` sets the field and returns the old value.
- **Extension traits**: Add the suffix `Ext`: `StrExt`, `PathExt`.
- **Lifetimes**: Use the same lifetime name everywhere that the same data occurs. Name the lifetime after the data it borrows, not after the type. Use `'_` when elision is possible. Do not hide a lifetime: write `Parser<'_>`, not `Parser`.
- **Builders**: The type `Config` gets `Config::builder()`, which returns `ConfigBuilder`. `ConfigBuilder` has no public constructor. `ConfigBuilder::build()` returns `Result<Config>` if a field can be invalid.

## 6. Ownership and borrowing

- Prefer unique ownership and borrows. Use `Rc` or `Arc` only when the borrow-based design is much more complex. Clone of an `Rc` or `Arc` is cheap and correct.
- Do not `.clone()` to make the borrow checker stop. First, try to change the scope or the data flow. Clone only when you need a real copy.
- For read-only parameters, accept the most general borrowed form:

| Do not accept | Accept |
|---|---|
| `&String` | `&str` |
| `&Vec<T>` | `&[T]` |
| `&Box<T>` | `&T` |
| `&Option<T>` | `Option<&T>` |
| `&PathBuf` | `&Path` |

- If a function stores the value, take ownership (`String`, `Vec<T>`). Do not take a borrow and then clone it inside the function.
- In library code, accept `impl IntoIterator<Item = T>` when the function only iterates. Application code can accept a slice.
- In library code, return `impl Iterator<Item = T>` when it is simple. Return `Vec<T>` when callers need indexing or ownership, or in application code.
- Use `impl Into<T>` parameters only in library code, and only where the call site becomes much easier to read.

## 7. Types

- Use a newtype, not a type alias, when values have a different meaning: `struct UserId(u64);`, not `type UserId = u64;`.
- Use named fields, not tuple structs. Exception: a newtype with one field.
- Make invalid states impossible. Use an enum with data instead of a struct with many `Option` fields and a `kind` field.
- Use an enum instead of a `bool` parameter when the meaning of `true` is not clear at the call site.

```rust
// Incorrect: what does `true` mean?
render(&scene, true);

// Correct
render(&scene, Shading::Smooth);
```

- Use `#[non_exhaustive]` on a public enum or a public struct with public fields when other crates use it and it can get new variants or fields.
- Use `#[must_use]` on functions when it is a bug to ignore the return value. Use it on error types and on lazy computations.

## 8. Traits

- Implement the standard trait when the behavior is the same. Use `FromStr`, `Display`, `From`, `TryFrom`, `Default`, `Iterator`, not a custom method with the same function.
- Use `#[derive(...)]` when derive gives the correct behavior. Write one `#[derive]` attribute. Put `Copy` first. Then put standard traits in alphabetical order. Then put third-party traits in alphabetical order.

```rust
#[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
struct Point { x: i32, y: i32 }
```

- Implement these traits when the conditions are true:
  - `Debug`: on all public types.
  - `Clone`: on most types. Not on handles to external resources.
  - `Copy`: only if the type will stay `Copy` in the future.
  - `Default`: when the type has `new()` without arguments, or a natural default state.
  - `PartialEq`, `PartialOrd`: by default. Add `Eq`, `Ord` only when you need the stronger guarantee.
  - `Hash` and `Eq` (and `Ord` if a total order exists): for map keys.
- Do not reorder struct fields to get a correct derived `Ord`. Write the `Ord` implementation by hand.
- Define a trait only for one of these purposes:
  - Generic code.
  - Accept or return `impl Trait` or `dyn Trait`.
  - Add methods to a foreign type (an extension trait).
- Do not use a trait only to organize methods. Use an inherent `impl` block.
- In library code, prefer generics (`T: Trait`, `impl Trait`) to `dyn Trait`. Use `dyn Trait` when binary size or compile time is more important, or for heterogeneous collections.
- Put the `impl Type` block in the same file as the type, immediately below the type.
- Use `Self` in `impl` blocks wherever possible.
- Keep all generic bounds in one location. If one bound goes into a `where` clause, put all bounds there.
- Use operator overloading only for mathematical or logical meaning.

## 9. Errors and panics

### 9.1 Choose the error mechanism

| Situation | Use |
|---|---|
| The only failure is "no value", and the cause is clear | `Option<T>` |
| The caller can recover and needs to know the cause | `Result<T, E>` |
| A bug: an invariant is broken | `panic!` |
| Bad user input, I/O failure, network failure | `Result`. Never panic. |

### 9.2 Error types

- **Library crates**: define an enum error type with `thiserror`. Do not expose `Box<dyn Error>` or `anyhow::Error` in a library API.
- **Application crates**: you can use `anyhow` (or `eyre`) in `main` and in top-level command code. Add context with `.context("cannot open config file")`.
- Name the field that wraps another error `source`.
- Do not expose the error types of dependencies in a public API. Wrap them in an opaque variant.
- Convert external errors to your crate error type early, with `?` and a `From` implementation, or with `map_err`.

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("cannot read config file {path}")]
    ReadConfig { path: PathBuf, #[source] source: io::Error },

    #[error("invalid port {port}: must be between 1 and 65535")]
    InvalidPort { port: u32 },
}
```

### 9.3 Error messages

- Start with a lowercase letter. Use uppercase only for acronyms and proper names.
- Do not put a period at the end.
- Start with a verb when possible: `cannot open file`, `failed to parse header`.
- Do not repeat context that the wrapping error adds.

### 9.4 Panics

- Do not use `.unwrap()` in production code. Use `?`, `ok_or`, `ok_or_else`, `let ... else`, or `match`.
- If a panic is correct, use `.expect("reason")`. Write the reason as the condition that must be true: `.expect("regex is valid at compile time")`.
- Start a panic message for an internal fault with `internal error:`.
- Do not use panics for control flow. With `panic = "abort"`, cleanup code does not run.
- Do not use `catch_unwind` in your own code.
- Tests can use `unwrap()` and can panic.
- Write `assert!` messages that explain the expected state and show the values. Do not repeat the condition.

```rust
assert!(index < self.len(), "index {index} is out of bounds for length {}", self.len());
```

## 10. Control flow and expressions

- Use pattern matching, not a check and then an unwrap.

```rust
// Incorrect
if value.is_some() { use_it(value.unwrap()); }

// Correct
if let Some(value) = value { use_it(value); }
```

- Use `let ... else` for an early return.

```rust
let Some(user) = users.get(&id) else {
    return Err(Error::UnknownUser { id });
};
```

- Match exhaustively on enums that your crate owns. Then a new variant causes a compile error, and you must review each `match`. Do not use `_ =>` for your own enums.
- For a `#[non_exhaustive]` enum from another crate, list the variants you use and add `_ =>` for the rest.
- Do not destructure tuples with `.0` and `.1`. Bind names: `let (width, height) = size;`. Exception: newtypes.
- Do not destructure parameters in a function signature. Destructure in the first line of the body. Closures can destructure parameters.
- Do not pattern-match references in closures. Write `|x| *x`, not `|&x| x`.
- Do not declare a `let` without a value and assign it later. Make the block return the value.

```rust
// Incorrect
let label;
if count == 1 { label = "item"; } else { label = "items"; }

// Correct
let label = if count == 1 { "item" } else { "items" };
```

- Keep `let mut` in the smallest scope. Build the value in a block, then bind it immutably with the same name.
- Shadow a variable with a different type at most one time (for example, a parse step). Never shadow a type name.
- Use iterator chains for transforms. Keep chains short. Do not put a long chain in a `for` header. Assign it to a variable first. Do not use `.for_each()` when a `for` loop is clearer.
- Use `.collect()`, not `Vec::from_iter()`.
- Use `Vec::new()` or `Vec::with_capacity(n)`. Do not use `vec![]` for an empty vector.
- Use `format!` and `write!` with inline arguments: `format!("{path}: {count}")`.
- Do not chain a method call on a closing `}`. Bind the block result to a variable first.
- End a statement that returns `()` with a semicolon.
- When a function returns `Result<()>`, end it with `call()?; Ok(())`, not with `call()`.
- Write empty match arms as `=> {}`, not `=> ()`.
- Declare types on `let` bindings, not with turbofish, when the type is not clear. Always show the target type of `.into()`, `.parse()`, and `.collect()` if the type is not clear from context.

## 11. Concurrency and async

- Prefer message passing (channels) to shared state.
- When you must share state, use `Arc<Mutex<T>>` or `Arc<RwLock<T>>`. Keep the lock for the shortest time. Do not hold a `std::sync::Mutex` guard across an `.await`.
- Do not access the data in a poisoned mutex. Propagate the panic.
- Use one async runtime in an application. Do not mix runtimes.
- Do not call blocking functions (file I/O, `std::thread::sleep`, heavy computation) inside an async task. Use `spawn_blocking` or a dedicated thread.
- Do not run code before `main()` (for example, with the `ctor` crate).

## 12. Unsafe code

Use `unsafe` only when no safe alternative exists.

1. Put `unsafe` code in a small module, or in a separate crate, behind a safe API.
2. Make each `unsafe` block as small as possible.
3. Write a `// SAFETY:` comment above each `unsafe` block. The comment explains why each precondition is true at this location.
4. Write a `# Safety` section in the doc comment of each `unsafe fn`. The section lists the preconditions for the caller.
5. Do not use `unsafe` only for speed. Use it for speed only when a benchmark shows a significant gain.
6. Run the tests under Miri (`cargo +nightly miri test`) for crates with `unsafe` code.

```rust
// SAFETY: `index` was checked against `self.len()` on the line above,
// so the pointer is in bounds and points to an initialized element.
let item = unsafe { self.items.get_unchecked(index) };
```

## 13. Macros

Use the first option that works, in this order:

1. Normal Rust code (functions, generics, traits).
2. `macro_rules!`.
3. A build script or other code generation.
4. A procedural macro.

Macros make IDE support, search, and review more difficult. Use them only when they remove a large quantity of repeated code.

## 14. Documentation

- Write a `///` doc comment for every public item. Write a `//!` comment at the top of each crate and each important module.
- The first sentence tells the reader when to use the item. Keep it to two lines or fewer.
- Write these sections when they apply: `# Errors`, `# Panics`, `# Safety`, `# Examples`.
- Add a runnable example in the crate documentation. Doc examples are tests. Make sure that they compile.
- Do not write a doc comment that only repeats the item name.
- Refer to parameters by name. Use "the", not "a", when you refer to a specific value.
- Use `//` for implementation comments.

## 15. Testing

- Put unit tests in a `#[cfg(test)] mod tests` block at the end of the file. Use `use super::*;` in it.
- Put integration tests in the `tests/` directory. These tests use only the public API.
- Give each test a name that says the expected behavior: `parse_returns_error_for_empty_input`.
- Test the error paths, not only the success path.
- Use property-based tests (`proptest`) for parsers and for functions with large input domains.
- Use `cargo nextest` if the project already uses it.

## 16. Dependencies and project hygiene

- Add a dependency only when it solves a real problem. Check that it is maintained.
- Set `rust-version` (MSRV) in `Cargo.toml` for library crates.
- Commit `Cargo.lock` for applications. For libraries, follow the project convention.
- Disable default features of a dependency when you do not need them.
- Run `cargo audit` or `cargo deny check` in CI.
- Use the `tracing` crate for logs in new projects. Do not use `println!` for diagnostics. Use `eprintln!` only in small command-line tools.

## 17. Review checklist

Before you finish a Rust change, make sure that each item is true:

- [ ] `cargo fmt`, `cargo clippy` (no warnings), and `cargo test` pass.
- [ ] There is no `.unwrap()` in production code. Each `.expect()` has a reason.
- [ ] Library errors are `thiserror` enums. There is no `anyhow` in library APIs.
- [ ] Parameters use `&str`, `&[T]`, `&Path`, and `Option<&T>`.
- [ ] There is no unnecessary `.clone()`, `Rc`, or `Arc`.
- [ ] There are no glob imports, except the permitted cases.
- [ ] Each `unsafe` block has a `// SAFETY:` comment.
- [ ] Public items have doc comments with `# Errors`, `# Panics`, and `# Safety` where they apply.
- [ ] New enums of the crate are matched exhaustively.

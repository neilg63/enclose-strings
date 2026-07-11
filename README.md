[![mirror](https://img.shields.io/badge/mirror-github-blue)](https://github.com/neilg63/enclose-strings)
[![crates.io](https://img.shields.io/crates/v/enclose-strings.svg)](https://crates.io/crates/enclose-strings)
[![docs.rs](https://docs.rs/to_segments/badge.svg)](https://docs.rs/enclose-strings)

# enclose-strings

Wrap or enclose strings in matching or complementary characters with optional escaping.

## Features

- `SimpleEnclose` trait, implemented for any `AsRef<str>` type (`&str`, `String`, etc.) via a blanket impl — no wrapper type needed.
- Core method `enclose_in_chars(start, end, prefix, escape_char)` wraps a string in arbitrary start/end characters, with an optional prefix inserted right after the opening character and optional escaping of the end character.
- `wrap` / `wrap_safe` / `wrap_escaped` enclose in a single opening character and automatically resolve the matching closing character for:
  - ASCII bracket pairs: `(` `)`, `<` `>`, `{` `}`, `[` `]`
  - Unicode quotation marks: curly single/double quotes, low/high-reversed quotes, guillemets, CJK corner and white corner brackets, small form variants, etc.
  - Any other character, which is treated as its own closing character (e.g. `"`, `'`, `` ` ``)
- Convenience shorthands: `parenthesize` / `parenthesize_safe`, `double_quotes` / `double_quotes_safe`, `single_quotes` / `single_quotes_safe`, and `in_parentheses` for wrapping with an inserted prefix (e.g. building regex lookaheads like `(?=...)`).
- Escaping (`escape_in_str`) is idempotent against already-escaped input and supports two styles:
  - Backslash-style, where `escape_char != end`: inserts the escape character before an unescaped occurrence of `end`, counting preceding escape characters so `\"` is left as `\"` rather than becoming `\\"`.
  - CSV/doubling-style, where `escape_char == end`: doubles a lone `end` character (`"` → `""`) without re-doubling a pair that's already doubled.
- No dependencies, and no allocation-heavy abstractions — just string wrapping/escaping helpers meant to complement other crates.

## Example

```rust
use enclose_strings::SimpleEnclose;

assert_eq!("purple".parenthesize(), "(purple)");
assert_eq!("hello".wrap('\u{201C}'), "\u{201C}hello\u{201D}"); // “hello”
assert_eq!(r#"say \"hi\""#.double_quotes_safe(), r#""say \"hi\"""#); // pre-escaped content is left untouched
```

Related crates:

- [**to_segments**](https://crates.io/crates/to_segments) — Provides the `ToSegments` trait for ergonomic string splitting with readable methods for common manipulation tasks. Also used internally by `alphanumeric`.
- [**simple-string-patterns**](https://crates.io/crates/simple-string-patterns) — Adds a set of simple string matching methods such case-insensitive and alphanumeric-only versions of starts_with, contains and ends_with as well as more complex rule sets without regular expressions.
- [**substring-replace**](https://crates.io/crates/substring-replace) — Provides methods to match and replace string slices as specified character indices.

This functionality was lifted out of [`simple-string-patterns`](https://crates.io/crates/simple-string-patterns) into its own zero-dependency crate so it can be used on its own to supplement other text-manipulation crates.

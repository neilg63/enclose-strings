[![mirror](https://img.shields.io/badge/mirror-github-blue)](https://github.com/neilg63/enclose-strings)
[![crates.io](https://img.shields.io/crates/v/enclose-strings.svg)](https://crates.io/crates/enclose-strings)
[![docs.rs](https://docs.rs/enclose-strings/badge.svg)](https://docs.rs/enclose-strings)

# enclose-strings

Wrap, escape and extract strings enclosed in matching or complementary delimiters — parentheses, quotes, brackets, XML comments, or any custom pair — with no dependencies.

## Quick start

```rust
use enclose_strings::SimpleEnclose;

// wrap in matched pairs
assert_eq!("purple".parenthesize(), "(purple)");
assert_eq!("purple".wrap('<'), "<purple>");
assert_eq!("purple".double_quotes(), r#""purple""#);

// works on String too, not just &str
let owned = String::from("world");
assert_eq!(owned.parenthesize(), "(world)");
```

## Features

- **Blanket impl** — `SimpleEnclose` is implemented for any `T: AsRef<str>`, so `&str`, `String`, `Cow<str>` and `Box<str>` all work directly, including in generic bounds.
- **Multi-character delimiters** — start and end may each be a `char`, `&str` or `String` via the `Delimiter` trait, so `"<!--"` / `"-->"` and `"(?="` / `')'` work without a separate prefix argument.
- **Explicit escape styles** — `EscapeStyle::Char('\\')` for backslash escaping, `EscapeStyle::Doubled` for CSV-style doubling, or `EscapeStyle::None` to leave content untouched. Escaping is idempotent: already-escaped content is left as-is.
- **Unicode quote pairs** — `wrap('(')` auto-resolves `)`, and the same works for curly quotes `" "`, guillemets `« »`, CJK corner brackets `「 」`, and many more.
- **Extraction** (opt-in `extract` feature) — the `SimpleExtract` trait is the inverse of `SimpleEnclose`: extract content from inside delimiters, with support for quoted regions, balanced nesting, and escape handling.

## Wrapping and enclosing

### Basic wrapping

`wrap(opening)` auto-resolves the matching closing character:

```rust
use enclose_strings::SimpleEnclose;

assert_eq!("x".wrap('('), "(x)");
assert_eq!("x".wrap('['), "[x]");
assert_eq!("x".wrap('{'), "{x}");
assert_eq!("x".wrap('<'), "<x>");
assert_eq!("x".wrap('"'), r#""x""#);  // same char closes
```

### Unicode quote pairs

```rust
use enclose_strings::SimpleEnclose;

assert_eq!("hello".wrap('\u{201C}'), "\u{201C}hello\u{201D}"); // "hello"
assert_eq!("hello".wrap('\u{00AB}'), "\u{00AB}hello\u{00BB}"); // «hello»
assert_eq!("hello".wrap('\u{300C}'), "\u{300C}hello\u{300D}"); // 「hello」
```

Supported pairs include:

| Opening | Closing | Description |
|---------|---------|-------------|
| `(` | `)` | Parentheses |
| `<` `[` `{` | `>` `]` `}` | Angle, square, curly brackets |
| `'` U+2018 | `'` U+2019 | Curly single quotes |
| `"` U+201C | `"` U+201D | Curly double quotes |
| `‹` U+2039 | `›` U+203A | Single guillemets |
| `«` U+00AB | `»` U+00BB | Double guillemets |
| `❛` U+275B | `❜` U+275C | Heavy single comma ornaments |
| `❝` U+275D | `❞` U+275E | Heavy double comma ornaments |
| `„` U+201E | `"` U+201C | Low-9 double quote |
| `「` U+300C | `」` U+300D | CJK corner brackets |
| `『` U+300E | `』` U+300F | CJK white corner brackets |
| `｢` U+FF62 | `｣` U+FF63 | Halfwidth corner brackets |

Any character not in the table closes on itself (e.g. `` ` ``, `|`, `~`).

### Multi-character delimiters

Start and end can be `char`, `&str` or `String`:

```rust
use enclose_strings::SimpleEnclose;

assert_eq!("body".enclose("<!--", "-->"), "<!--body-->");
assert_eq!("purple".enclose("(?=", ')'), "(?=purple)");
assert_eq!("x".enclose("[[", "]]"), "[[x]]");
assert_eq!("x".enclose(String::from("<<"), String::from(">>")), "<<x>>");
```

### Prefix insertion

`in_parentheses` inserts a prefix right after the opening bracket — useful for regex groups:

```rust
use enclose_strings::SimpleEnclose;

assert_eq!("purple".in_parentheses(Some("?=")), "(?=purple)");
assert_eq!("purple".in_parentheses(None), "(purple)");
```

## Escaping

### Backslash escaping (`_safe` methods)

The `_safe` variants escape occurrences of the closing delimiter with a backslash:

```rust
use enclose_strings::SimpleEnclose;

let input = r#"Tom whispered "I love you" as he gazed into her eyes."#;
assert_eq!(
    input.double_quotes_safe(),
    r#""Tom whispered \"I love you\" as he gazed into her eyes.""#
);
```

Already-escaped content is left alone (idempotent):

```rust
use enclose_strings::SimpleEnclose;

let pre_escaped = r#"say \"hello\""#;
assert_eq!(pre_escaped.double_quotes_safe(), r#""say \"hello\"""#);
```

### CSV-style doubling

`EscapeStyle::Doubled` or the `_doubled` convenience doubles the closing delimiter instead of prefixing it:

```rust
use enclose_strings::{SimpleEnclose, EscapeStyle};

let input = r#"She wrote "Cold Heart""#;
assert_eq!(
    input.double_quotes_doubled(),
    r#""She wrote ""Cold Heart""""#
);
// equivalent to:
assert_eq!(
    input.wrap_escaped('"', EscapeStyle::Doubled),
    r#""She wrote ""Cold Heart""""#
);
```

### Explicit EscapeStyle

Use `enclose_escaped` or `wrap_escaped` with any `EscapeStyle`:

```rust
use enclose_strings::{SimpleEnclose, EscapeStyle};

// backslash-escape a multi-character closing delimiter
assert_eq!(
    "a --> b".enclose_safe("<!--", "-->"),
    r#"<!--a \--> b-->"#
);

// no escaping
assert_eq!(
    "content".wrap_escaped('"', EscapeStyle::None),
    r#""content""#
);
```

## Convenience methods

| Method | Equivalent |
|--------|-----------|
| `parenthesize()` | `wrap('(')` |
| `parenthesize_safe()` | `wrap_safe('(')` |
| `double_quotes()` | `wrap('"')` |
| `double_quotes_safe()` | `wrap_escaped('"', EscapeStyle::Char('\\'))` |
| `double_quotes_doubled()` | `wrap_escaped('"', EscapeStyle::Doubled)` |
| `single_quotes()` | `wrap('\'')` |
| `single_quotes_safe()` | `wrap_escaped('\'', EscapeStyle::Char('\\'))` |

## Extraction (feature: `extract`)

Enable the `extract` feature to get the inverse operation — extracting content from inside delimiters:

```toml
[dependencies]
enclose-strings = { version = "0.2", features = ["extract"] }
```

### Basic extraction

```rust
use enclose_strings::SimpleExtract;

assert_eq!("func(arg)".extract_from_parentheses(), Some("arg".to_string()));
assert_eq!(r#"say "hello" now"#.extract_from_double_quotes(), Some("hello".to_string()));
assert_eq!("a[0]".extract_from_brackets(), Some("0".to_string()));
assert_eq!("x{y}z".extract_from_braces(), Some("y".to_string()));
```

### Custom delimiters

```rust
use enclose_strings::{SimpleExtract, EscapeStyle};

assert_eq!(
    "<!--a comment-->tail".extract_first("<!--", "-->"),
    Some("a comment".to_string())
);
assert_eq!(
    "[[wiki link]]".extract_first("[[", "]]"),
    Some("wiki link".to_string())
);
```

### Escaped content

Escape characters are consumed during extraction, restoring the original content:

```rust
use enclose_strings::{SimpleExtract, EscapeStyle};

// backslash escaping: \) is content, not a closing delimiter
assert_eq!(
    r#"(a (nested\) call)"#.extract_enclosed('('),
    Some("a (nested) call".to_string())
);

// CSV doubling: "" inside quotes is a literal quote
assert_eq!(
    r#""She said ""hello""""#.extract_enclosed_doubled('"'),
    Some(r#"She said "hello""#.to_string())
);
```

### Segmenting a string

`extract_enclosures` splits the whole string into `CapturedSegment::Enclosure` and `CapturedSegment::Outside` pieces:

```rust
use enclose_strings::{SimpleExtract, CapturedSegment, EscapeStyle};

let segments = "prefix!(one)mid!(two)suffix"
    .extract_enclosures("!(", ')', EscapeStyle::None);

assert_eq!(segments[0], CapturedSegment::Outside("prefix"));
assert_eq!(segments[1].enclosure(), Some("one"));
assert_eq!(segments[2], CapturedSegment::Outside("mid"));
assert_eq!(segments[3].enclosure(), Some("two"));
```

### Balanced nesting and quoted regions

`extract_arguments` handles the common case of a function call: it tracks nested brackets and treats single/double quoted regions as opaque.

```rust
use enclose_strings::SimpleExtract;

let call = r#"add_title("Latest Stats (2024-2025)", "en-GB")"#;
assert_eq!(
    call.extract_arguments('('),
    Some(r#""Latest Stats (2024-2025)", "en-GB""#.to_string())
);

// nested brackets are balanced
assert_eq!(
    "f(g(x), y)".extract_arguments('('),
    Some("g(x), y".to_string())
);
```

For fine-grained control, use `ScanOptions`:

```rust
use enclose_strings::{SimpleExtract, ScanOptions, Nesting, EscapeStyle};

let options = ScanOptions::default()
    .with_quotes(&['"'])
    .with_nesting(Nesting::Delimiters)
    .with_escape(EscapeStyle::Char('\\'));

assert_eq!(
    r#"f("a )b", c)"#.extract_first_enclosure_with('(', ')', options),
    Some(r#""a )b", c"#.to_string())
);
```

### Round-trip

Wrapping and extracting are inverses:

```rust
use enclose_strings::{SimpleEnclose, SimpleExtract, EscapeStyle};

let original = r#"has ) and " inside"#;
let wrapped = original.wrap_safe('(');
assert_eq!(
    wrapped.extract_enclosed('('),
    Some(original.to_string())
);
```

## Generic usage

Because the trait is blanket-implemented, it works in generic contexts:

```rust
use enclose_strings::SimpleEnclose;

fn quote<T: SimpleEnclose>(val: T) -> String {
    val.double_quotes()
}

assert_eq!(quote("hello"), r#""hello""#);
assert_eq!(quote(String::from("hello")), r#""hello""#);
```

## Related crates

- [**to_segments**](https://crates.io/crates/to_segments) — Ergonomic string splitting with readable methods for common manipulation tasks.
- [**simple-string-patterns**](https://crates.io/crates/simple-string-patterns) — Simple string matching methods: case-insensitive and alphanumeric-only variants of `starts_with`, `contains` and `ends_with`, plus complex rule sets without regular expressions.
- [**alphanumeric**](https://crates.io/crates/alphanumeric) — International number format parsing with support for multiple separator conventions.
- [**substring-replace**](https://crates.io/crates/substring-replace) — Match and replace string slices at specified character indices.

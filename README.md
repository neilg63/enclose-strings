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
- **Unicode quote pairs** — `wrap('(')` auto-resolves `)`, and the same works for curly quotes `\u{201C} \u{201D}`, guillemets `\u{00AB} \u{00BB}`, CJK corner brackets `\u{300C} \u{300D}`, and many more.
- **Extraction** (opt-in `extract` feature) — the `SimpleExtract` trait is the inverse of `SimpleEnclose`: extract content from inside delimiters, with support for quoted regions, balanced nesting, and escape handling.

## Complementary vs. identical delimiters

The crate draws a fundamental distinction between two kinds of delimiter pair:

**Complementary pairs** have distinct opening and closing characters — `(` / `)`, `[` / `]`, `{` / `}`, `<` / `>`, and the Unicode pairs listed below. Because the two characters are different, nesting is unambiguous: each opening character increments a depth counter and each closing character decrements it, so `((a) and (b))` is one outer group containing two inner groups.

**Identical pairs** use the same character for both sides — `"`, `'`, `` ` ``, `|`, etc. There is no way to distinguish an "opening" from a "closing" occurrence, so nesting is inherently impossible. The only way to include the delimiter character inside the content is to *escape* it — with a backslash (`\"`) or by doubling (`""`).

This distinction affects which extraction strategy applies:

| Situation | Strategy | Method |
|-----------|----------|--------|
| Complementary pair, nested | Balance depth | `extract_balanced`, `extract_all_enclosed` |
| Complementary pair, escaped | Backslash / doubling | `extract_enclosed`, `extract_enclosed_doubled` |
| Identical pair | Escape only (nesting impossible) | `extract_enclosed`, `extract_enclosed_doubled` |

Both strategies produce a correct round-trip with their `SimpleEnclose` counterpart — see [Round-trip](#round-trip) below.

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

assert_eq!("hello".wrap('\u{201C}'), "\u{201C}hello\u{201D}"); // \u{201C}hello\u{201D}
assert_eq!("hello".wrap('\u{00AB}'), "\u{00AB}hello\u{00BB}"); // \u{00AB}hello\u{00BB}
assert_eq!("hello".wrap('\u{300C}'), "\u{300C}hello\u{300D}"); // \u{300C}hello\u{300D}
```

Supported pairs include:

| Opening | Closing | Description |
|---------|---------|-------------|
| `(` | `)` | Parentheses |
| `<` `[` `{` | `>` `]` `}` | Angle, square, curly brackets |
| `\u{2018}` | `\u{2019}` | Curly single quotes |
| `\u{201C}` | `\u{201D}` | Curly double quotes |
| `\u{2039}` | `\u{203A}` | Single guillemets |
| `\u{00AB}` | `\u{00BB}` | Double guillemets |
| `\u{275B}` | `\u{275C}` | Heavy single comma ornaments |
| `\u{275D}` | `\u{275E}` | Heavy double comma ornaments |
| `\u{201E}` | `\u{201C}` | Low-9 double quote |
| `\u{300C}` | `\u{300D}` | CJK corner brackets |
| `\u{300E}` | `\u{300F}` | CJK white corner brackets |
| `\u{FF62}` | `\u{FF63}` | Halfwidth corner brackets |

Any character not in the table closes on itself (e.g. `"`, `'`, `` ` ``, `|`, `~`).

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
use enclose_strings::SimpleExtract;

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
use enclose_strings::SimpleExtract;

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

### Nested enclosure pairs

When the opening and closing characters differ, `extract_all_enclosed` tracks balanced depth and returns every group at every nesting level, ordered by opening position (outermost first):

```rust
use enclose_strings::SimpleExtract;

let nested = "((red or blue) and (green or orange))";
assert_eq!(
    nested.extract_all_enclosed('('),
    vec![
        "(red or blue) and (green or orange)".to_string(),
        "red or blue".to_string(),
        "green or orange".to_string(),
    ]
);

// same for square brackets
let nested_sq = "[[a or b] and [c or d]]";
assert_eq!(
    nested_sq.extract_all_enclosed('['),
    vec![
        "[a or b] and [c or d]".to_string(),
        "a or b".to_string(),
        "c or d".to_string(),
    ]
);

// deeply nested: outermost first, then each inner level
assert_eq!(
    "(a(b(c)))".extract_all_enclosed('('),
    vec!["a(b(c))".to_string(), "b(c)".to_string(), "c".to_string()]
);
```

For identical delimiters like `"` or `'`, nesting is impossible — each occurrence is ambiguous between opening and closing — so `extract_all_enclosed` falls back to a flat scan with backslash escaping:

```rust
use enclose_strings::SimpleExtract;

assert_eq!(
    r#""one" and "two""#.extract_all_enclosed('"'),
    vec!["one".to_string(), "two".to_string()]
);
```

### Balanced nesting and quoted regions

The nesting-aware methods track balanced depth and treat single/double quoted regions as opaque, so closing delimiters inside nested groups or quoted strings are content, not terminators. They come in three shapes:

| Method | Returns | Description |
|--------|---------|-------------|
| `extract_balanced(opening)` | `Option<String>` | First balanced enclosure content |
| `extract_nth_balanced(opening, n)` | `Option<String>` | *n*th balanced enclosure (0-indexed) |
| `extract_all_balanced(opening)` | `Vec<String>` | All balanced enclosure contents |
| `extract_captures(opening)` | `Vec<CapturedSegment>` | Full segmented view |

```rust
use enclose_strings::SimpleExtract;

let expr = "f(g(x), y) + h(z)";

// first balanced enclosure
assert_eq!(
    expr.extract_balanced('('),
    Some("g(x), y".to_string())
);

// nth balanced enclosure (0-indexed)
assert_eq!(expr.extract_nth_balanced('(', 0), Some("g(x), y".to_string()));
assert_eq!(expr.extract_nth_balanced('(', 1), Some("z".to_string()));
assert_eq!(expr.extract_nth_balanced('(', 2), None);

// all balanced enclosure contents
assert_eq!(
    expr.extract_all_balanced('('),
    vec!["g(x), y".to_string(), "z".to_string()]
);
```

Quoted regions beat nesting — a closing delimiter inside quotes is content:

```rust
use enclose_strings::SimpleExtract;

let call = r#"add_title("Latest Stats (2024-2025)", "en-GB")"#;
assert_eq!(
    call.extract_balanced('('),
    Some(r#""Latest Stats (2024-2025)", "en-GB""#.to_string())
);

assert_eq!(
    r#"f("Smiley :)", "ok")"#.extract_balanced('('),
    Some(r#""Smiley :)", "ok""#.to_string())
);
```

### Captures: the segmented view

`extract_captures` splits a string into `CapturedSegment::Enclosure` and `CapturedSegment::Outside` pieces with full nesting and quoting support:

```rust
use enclose_strings::{SimpleExtract, CapturedSegment};

let segments = "f(a, b) + g(c, d)".extract_captures('(');

assert_eq!(segments[0], CapturedSegment::Outside("f"));
assert_eq!(segments[1].enclosure(), Some("a, b"));
assert_eq!(segments[2], CapturedSegment::Outside(" + g"));
assert_eq!(segments[3].enclosure(), Some("c, d"));
```

Convenience methods for the most common bracket types:

```rust
use enclose_strings::{SimpleExtract, CapturedSegment};

let parens = "f(a) + g(b)".extract_all_from_parentheses();
assert_eq!(parens[1].enclosure(), Some("a"));

let brackets = "a[0] + b[1]".extract_all_from_brackets();
assert_eq!(brackets[1].enclosure(), Some("0"));

let braces = "x{m} + y{n}".extract_all_from_braces();
assert_eq!(braces[1].enclosure(), Some("m"));
```

For a flat scan (no nesting, no quoted regions), `extract_segments` and `extract_enclosures` remain available:

```rust
use enclose_strings::{SimpleExtract, CapturedSegment, EscapeStyle};

let segments = "prefix!(one)mid!(two)suffix"
    .extract_enclosures("!(", ')', EscapeStyle::None);

assert_eq!(segments[0], CapturedSegment::Outside("prefix"));
assert_eq!(segments[1].enclosure(), Some("one"));
assert_eq!(segments[2], CapturedSegment::Outside("mid"));
assert_eq!(segments[3].enclosure(), Some("two"));
```

### Fine-grained control with ScanOptions

For cases beyond the defaults, `ScanOptions` lets you choose the escape style, which quote characters are opaque, and the nesting strategy:

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

### Parsing a mini-DSL

These extraction methods compose naturally with standard string splitting for parsing lightweight domain-specific syntaxes. For example, given a key spec like `valid_codes|int[]:(,)`:

```rust
use enclose_strings::SimpleExtract;

let spec = "valid_codes|int[]:(,)";

// split_once peels off the field name and the cast
let (field, rest) = spec.split_once('|').unwrap();
assert_eq!(field, "valid_codes");

let (cast, tail) = rest.split_once(':').unwrap();
assert_eq!(cast, "int[]");

// extract_enclosed pulls the separator from inside the parentheses
assert_eq!(tail.extract_enclosed('('), Some(",".to_string()));
```

A two-stage parse can first extract the balanced argument list, then pull out the individual quoted values:

```rust
use enclose_strings::SimpleExtract;

let call = r#"add_title("Latest Stats (2024-2025)", "en-GB")"#;

// extract_balanced handles nested brackets and quoted regions
let args = call.extract_balanced('(').unwrap();
assert_eq!(args, r#""Latest Stats (2024-2025)", "en-GB""#);

// extract_all_enclosed on quotes does a flat scan (quotes can't nest)
assert_eq!(
    args.extract_all_enclosed('"'),
    vec!["Latest Stats (2024-2025)".to_string(), "en-GB".to_string()]
);
```

### Round-trip

Wrapping and extracting are inverses. Two strategies both recover the original:

```rust
use enclose_strings::{SimpleEnclose, SimpleExtract};

let original = "(red or blue) and (green or orange)";

// Strategy 1: escape the inner brackets, then extract with escaping
let escaped = original.wrap_safe('(');
assert_eq!(escaped, r#"((red or blue\) and (green or orange\))"#);
assert_eq!(escaped.extract_enclosed('('), Some(original.to_string()));

// Strategy 2: wrap literally, then extract with balanced nesting
let literal = original.wrap('(');
assert_eq!(literal, "((red or blue) and (green or orange))");
assert_eq!(literal.extract_balanced('('), Some(original.to_string()));
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

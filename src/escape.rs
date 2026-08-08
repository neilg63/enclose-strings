use crate::delimiter::Delimiter;

/// How occurrences of the closing delimiter are protected inside the content.
///
/// This replaces an `Option<char>` that had to carry two meanings at once: an
/// escape character equal to the end character used to switch silently to the
/// doubling convention. Stating the convention removes that coincidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EscapeStyle {
    /// Leave the content alone. Any closing delimiter inside it will terminate
    /// the value early when read back.
    #[default]
    None,
    /// Prefix the delimiter with an escape character, `\)` style. An occurrence
    /// already preceded by an odd number of escape characters is left as it is,
    /// so escaping is idempotent.
    Char(char),
    /// Repeat the delimiter, `""` style, as CSV does. An already-doubled pair is
    /// left as it is.
    Doubled,
}

impl From<Option<char>> for EscapeStyle {
    /// Convenience for callers migrating from the old `Option<char>` argument.
    /// Note that `Some(c)` always means [`EscapeStyle::Char`] now, even where
    /// `c` equals the end delimiter — ask for [`EscapeStyle::Doubled`] instead.
    fn from(value: Option<char>) -> Self {
        match value {
            Some(escape_char) => EscapeStyle::Char(escape_char),
            None => EscapeStyle::None,
        }
    }
}

/// Protect every occurrence of `end` within `source` according to `style`.
pub fn escape_delimiter<E: Delimiter>(source: &str, end: &E, style: EscapeStyle) -> String {
    if matches!(style, EscapeStyle::None) || end.is_empty() {
        return source.to_owned();
    }
    let mut out = String::with_capacity(source.len() + 8);
    let mut index = 0;
    // Consecutive escape characters seen in the source immediately before the
    // current position. An odd count means the next delimiter is already
    // escaped.
    let mut pending_escapes = 0usize;

    while index < source.len() {
        if let Some(width) = end.match_len(source, index) {
            match style {
                EscapeStyle::Char(escape_char) => {
                    if pending_escapes.is_multiple_of(2) {
                        out.push(escape_char);
                    }
                    out.push_str(&source[index..index + width]);
                    index += width;
                }
                EscapeStyle::Doubled => {
                    let already_doubled = end.match_len(source, index + width).is_some();
                    out.push_str(&source[index..index + width]);
                    out.push_str(&source[index..index + width]);
                    index += if already_doubled { width * 2 } else { width };
                }
                EscapeStyle::None => unreachable!("handled above"),
            }
            pending_escapes = 0;
            continue;
        }

        let character = source[index..].chars().next().expect("index on a boundary");
        if let EscapeStyle::Char(escape_char) = style {
            pending_escapes = if character == escape_char {
                pending_escapes + 1
            } else {
                0
            };
        }
        out.push(character);
        index += character.len_utf8();
    }
    out
}

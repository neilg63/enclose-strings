use crate::escape::EscapeStyle;

/// Whether the scanner tracks nested openings, and on which pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Nesting {
    /// The first closing delimiter ends the content.
    #[default]
    None,
    /// Count the opening and closing delimiters as given, so `f(g(x), y)`
    /// yields `g(x), y`. Inert when the two are identical, since nothing
    /// distinguishes an opening from a closing in that case.
    Delimiters,
    /// Count a different pair from the bounding delimiters. Needed when the
    /// opening delimiter carries a prefix: with `"(?<="` as the opening and
    /// `')'` as the closing, `Pair('(', ')')` still balances the inner groups
    /// of `(?<=(a|b))`.
    Pair(char, char),
}

/// How the scanner decides which closing delimiter really closes the content.
///
/// The default is a plain scan: no escaping, no quoted regions, no nesting, so
/// the first closing delimiter wins. Build on it with the struct-update syntax
/// or the builder methods:
///
/// ```
/// # use enclose_strings::{EscapeStyle, Nesting, ScanOptions};
/// let options = ScanOptions::default()
///     .with_quotes(&['"'])
///     .with_nesting(Nesting::Delimiters);
/// # assert_eq!(options.escape, EscapeStyle::None);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ScanOptions<'a> {
    /// How an escaped delimiter is written, if escaping is used at all.
    pub escape: EscapeStyle,
    /// Characters that open and close a quoted region. A closing delimiter
    /// inside such a region is part of the content, not the end of it, which is
    /// what lets `("a (b) c", "d")` be read as one argument list. Each quote
    /// closes on its own character, and `escape` applies inside the region, so
    /// an escaped quote does not end it.
    pub quotes: &'a [char],
    /// Whether nested openings are counted, and on which pair.
    pub nesting: Nesting,
}

impl<'a> ScanOptions<'a> {
    /// A scan with no escaping, no quoted regions and no nesting.
    pub fn plain() -> Self {
        Self::default()
    }

    /// A scan using the given escape style.
    pub fn escaped(escape: EscapeStyle) -> Self {
        Self {
            escape,
            ..Self::default()
        }
    }

    /// A scan that treats single and double quoted regions as opaque and
    /// counts nested delimiters, which is the usual requirement when the
    /// delimiters bound an argument list.
    pub fn quoted() -> Self {
        Self {
            escape: EscapeStyle::Char('\\'),
            quotes: &['"', '\''],
            nesting: Nesting::Delimiters,
        }
    }

    pub fn with_escape(mut self, escape: EscapeStyle) -> Self {
        self.escape = escape;
        self
    }

    pub fn with_quotes(mut self, quotes: &'a [char]) -> Self {
        self.quotes = quotes;
        self
    }

    pub fn with_nesting(mut self, nesting: Nesting) -> Self {
        self.nesting = nesting;
        self
    }
}

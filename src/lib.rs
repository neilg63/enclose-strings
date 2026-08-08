mod delimiter;
mod escape;
mod match_closing;
mod scan;
#[cfg(feature = "extract")]
mod unwrap;

pub use delimiter::Delimiter;
pub use escape::{escape_delimiter, EscapeStyle};
pub use match_closing::match_closing_char;
pub use scan::{Nesting, ScanOptions};
#[cfg(feature = "extract")]
pub use unwrap::{CapturedSegment, SimpleExtract};

/// Traits with extension methods to wrap strings in bounding characters
pub trait SimpleEnclose {
    /// The single method an implementor must provide. Everything else in this
    /// trait is built on it, so prefer [`SimpleEnclose::enclose_escaped`] and
    /// the named conveniences at a call site.
    ///
    /// `start` and `end` may each be a `char`, a `&str` or a `String`. A
    /// multi-character opening such as `"(?="` carries any prefix itself, so no
    /// separate prefix argument is needed.
    fn enclose_in_chars<S: Delimiter, E: Delimiter>(
        &self,
        start: S,
        end: E,
        escape: EscapeStyle,
    ) -> String;

    /// Enclose in a start and an end delimiter with the given escape style.
    ///
    /// This is a common syntactical pattern in many markup and programming
    /// languages. The [`EscapeStyle`] decides how occurrences of `end` inside
    /// the content are protected; escaping is idempotent, so already-escaped
    /// content is left as it is.
    fn enclose_escaped<S: Delimiter, E: Delimiter>(
        &self,
        start: S,
        end: E,
        escape: EscapeStyle,
    ) -> String {
        self.enclose_in_chars(start, end, escape)
    }

    /// Enclose in a start and an end delimiter, leaving the content untouched
    fn enclose<S: Delimiter, E: Delimiter>(&self, start: S, end: E) -> String {
        self.enclose_in_chars(start, end, EscapeStyle::None)
    }

    /// Enclose in a start and an end delimiter, escaping the closing one with a
    /// backslash where it occurs in the content
    fn enclose_safe<S: Delimiter, E: Delimiter>(&self, start: S, end: E) -> String {
        self.enclose_in_chars(start, end, EscapeStyle::Char('\\'))
    }

    /// Wrap a string in a pair of characters, with the closing character matching the first character
    /// if it is a parenthesis (round bracket), angle bracket, (square) bracket or curly brace. Otherwise
    /// the closing character will be the same opening character.
    fn wrap_escaped(&self, opening: char, escape: EscapeStyle) -> String {
        let end = match_closing_char(opening);
        self.enclose_in_chars(opening, end, escape)
    }

    /// wrap a string in matching opening and closing characters
    fn wrap(&self, opening: char) -> String {
        let end = match_closing_char(opening);
        self.enclose_in_chars(opening, end, EscapeStyle::None)
    }

    /// Wrap in matching characters escaped by a backslash \
    fn wrap_safe(&self, opening: char) -> String {
        let end = match_closing_char(opening);
        self.enclose_in_chars(opening, end, EscapeStyle::Char('\\'))
    }

    /// Wrap in matching characters using the doubling convention, as CSV does
    fn wrap_doubled(&self, opening: char) -> String {
        let end = match_closing_char(opening);
        self.enclose_in_chars(opening, end, EscapeStyle::Doubled)
    }

    /// wrap in parentheses (round brackets) with an optional prefix before the
    /// main content, e.g. `Some("?=")` gives `(?=content)`
    fn in_parentheses(&self, prefix: Option<&str>) -> String {
        match prefix {
            Some(pre) => self.enclose_in_chars(format!("({pre}"), ')', EscapeStyle::None),
            None => self.enclose_in_chars('(', ')', EscapeStyle::None),
        }
    }

    /// wrap in parentheses (round brackets)
    fn parenthesize(&self) -> String {
        self.wrap('(')
    }

    /// wrap in parentheses (round brackets), escaping any in the content
    fn parenthesize_safe(&self) -> String {
        self.wrap_safe('(')
    }

    /// wrap in double quotes
    fn double_quotes(&self) -> String {
        self.wrap('"')
    }

    /// Wrap in single quotes
    fn single_quotes(&self) -> String {
        self.wrap('\'')
    }

    /// Wrap in double quotes with escaped quotes in the content
    fn double_quotes_safe(&self) -> String {
        self.wrap_escaped('"', EscapeStyle::Char('\\'))
    }

    /// Wrap in single quotes with escaped quotes in the content
    fn single_quotes_safe(&self) -> String {
        self.wrap_escaped('\'', EscapeStyle::Char('\\'))
    }

    /// Wrap in double quotes, doubling any in the content, as CSV does
    fn double_quotes_doubled(&self) -> String {
        self.wrap_doubled('"')
    }
}

impl<T: AsRef<str>> SimpleEnclose for T {
    fn enclose_in_chars<S: Delimiter, E: Delimiter>(
        &self,
        start: S,
        end: E,
        escape: EscapeStyle,
    ) -> String {
        let mut out = escape_delimiter(self.as_ref(), &end, escape);
        start.insert_into(&mut out, 0);
        end.push_to(&mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enclose_in_chars() {
        let sample_str = "purple";

        assert_eq!(sample_str.parenthesize(), "(purple)");
        assert_eq!(sample_str.wrap('<'), "<purple>");
        assert_eq!(
            sample_str.enclose('\u{201F}', '\u{201E}'),
            "\u{201F}purple\u{201E}"
        );
        assert_eq!(sample_str.in_parentheses(Some("?=")), "(?=purple)");
    }

    #[test]
    fn test_multi_char_delimiters() {
        // a multi-character opening removes the need for a separate prefix
        assert_eq!("purple".enclose("(?=", ')'), "(?=purple)");
        assert_eq!("body".enclose("<!--", "-->"), "<!--body-->");
        // start and end may be different types
        assert_eq!("x".enclose('[', "]]"), "[x]]");
        // String delimiters work too
        assert_eq!("x".enclose(String::from("<<"), String::from(">>")), "<<x>>");
    }

    #[test]
    fn test_multi_char_delimiter_escaping() {
        // the whole closing token is escaped, not just its first character
        assert_eq!("a --> b".enclose_safe("<!--", "-->"), r#"<!--a \--> b-->"#);
    }

    #[test]
    fn test_enclose_escaped_in_chars() {
        let sample_str =
            r#"Tom whispered "I love you" as he gazed into Jennifer's eyes only inches away."#;

        let expected_quoted_str =
            r#""Tom whispered \"I love you\" as he gazed into Jennifer's eyes only inches away.""#;

        assert_eq!(sample_str.double_quotes_safe(), expected_quoted_str);

        let sample_str_2 = r#"Bee's wax and \'organic honey\'"#;
        let expected_quoted_str_2 = r#"'Bee\'s wax and \'organic honey\''"#;

        assert_eq!(sample_str_2.single_quotes_safe(), expected_quoted_str_2);

        let sample_str_3 = r#"She wrote "From Antarctica with a Cold Heart""#;
        let expected_quoted_str_3 = r#""She wrote ""From Antarctica with a Cold Heart""""#;

        assert_eq!(
            sample_str_3.wrap_escaped('"', EscapeStyle::Doubled),
            expected_quoted_str_3
        );
        // the named convenience does the same thing
        assert_eq!(sample_str_3.double_quotes_doubled(), expected_quoted_str_3);
    }

    #[test]
    fn test_escape_leading_end_char() {
        // end char at position 0 must be escaped
        assert_eq!(r#""hello"#.double_quotes_safe(), r#""\"hello""#);
    }

    #[test]
    fn test_escaped_backslash_before_quote() {
        // \\" = escaped backslash + bare quote — the quote must be escaped
        let input = r#"path\\to\\"#;
        let result = input.enclose_safe('"', '"');
        assert_eq!(result, r#""path\\to\\""#);
    }

    #[test]
    fn test_pre_escaped_quote_preserved() {
        // \" is already escaped — should not become \\"
        let input = r#"say \"hello\""#;
        assert_eq!(input.double_quotes_safe(), r#""say \"hello\"""#);
    }

    #[test]
    fn test_pre_escaped_single_quote_and_bracket_preserved() {
        // \' is already escaped and should stay a single backslash, not doubled
        let input = r#"it\'s already escaped"#;
        assert_eq!(input.single_quotes_safe(), r#"'it\'s already escaped'"#);

        // \) is already escaped, so wrap_safe must leave it alone despite the
        // unescaped '(' characters also present in the content
        let input_2 = r#"a (nested\) call"#;
        assert_eq!(input_2.wrap_safe('('), r#"(a (nested\) call)"#);
    }

    #[test]
    fn test_csv_doubling_idempotent() {
        // already-doubled quotes should not be quadrupled
        let input = r#"She said ""hello"""#;
        assert_eq!(
            input.wrap_escaped('"', EscapeStyle::Doubled),
            r#""She said ""hello""""#
        );
    }

    #[test]
    fn test_escape_style_from_option() {
        // migration helper for the old Option<char> argument
        assert_eq!(EscapeStyle::from(Some('\\')), EscapeStyle::Char('\\'));
        assert_eq!(EscapeStyle::from(None), EscapeStyle::None);
        // Some(c) is never inferred as Doubled, even where c is the end char
        assert_eq!(EscapeStyle::from(Some('"')), EscapeStyle::Char('"'));
    }

    #[test]
    fn test_unicode_quote_pairs() {
        let s = "hello";
        assert_eq!(s.wrap('\u{201C}'), "\u{201C}hello\u{201D}");
        assert_eq!(s.wrap('\u{2018}'), "\u{2018}hello\u{2019}");
        assert_eq!(s.wrap('\u{00AB}'), "\u{00AB}hello\u{00BB}");
        assert_eq!(s.wrap('\u{2039}'), "\u{2039}hello\u{203A}");
        assert_eq!(s.wrap('\u{275B}'), "\u{275B}hello\u{275C}");
        assert_eq!(s.wrap('\u{275D}'), "\u{275D}hello\u{275E}");
        assert_eq!(s.wrap('\u{300C}'), "\u{300C}hello\u{300D}");
        assert_eq!(s.wrap('\u{300E}'), "\u{300E}hello\u{300F}");
        assert_eq!(s.wrap('\u{FF62}'), "\u{FF62}hello\u{FF63}");
        // single-quote forms matching the double-quote pairs above
        assert_eq!(s.wrap('\u{201A}'), "\u{201A}hello\u{2018}");
        assert_eq!(s.wrap('\u{201B}'), "\u{201B}hello\u{201A}");
    }

    #[test]
    fn test_owned_string() {
        let owned = String::from("world");
        assert_eq!(owned.double_quotes(), "\"world\"");
        assert_eq!(owned.parenthesize(), "(world)");

        fn generic_enclose<T: SimpleEnclose>(val: T) -> String {
            val.wrap('"')
        }
        assert_eq!(generic_enclose(String::from("ok")), "\"ok\"");
        assert_eq!(generic_enclose("ok"), "\"ok\"");
    }
}

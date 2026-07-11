use std::str::Chars;

/// Traits with extension emthods to wrap strings in bounding characters
pub trait SimpleEnclose {
    /// Enclose in a start and an end character with an optional prefix
    /// before the main content but after the first character
    /// This is a common syntactical pattern in many markup and programming languages
    /// the closing character will be the same opening character
    /// The optional escape character is inserted before occurrences of the end character
    /// unless the preceding character is the escape character itself to avoid double escaping of pre-escaped strings
    fn enclose_in_chars(
        &self,
        start: char,
        end: char,
        prefix: Option<&str>,
        escape_char: Option<char>,
    ) -> String;

    /// Enclose in a start and an end character with an optional prefix after the first character
    fn enclose_escaped(&self, start: char, end: char, escape_char: Option<char>) -> String {
        self.enclose_in_chars(start, end, None, escape_char)
    }

    /// Enclose in a start and an end character with an optional prefix after the first character
    fn enclose(&self, start: char, end: char) -> String {
        self.enclose_in_chars(start, end, None, None)
    }

    /// Enclose in a start and an end character with an optional prefix after the first character
    /// escaped where necessary with a backslash \
    fn enclose_safe(&self, start: char, end: char) -> String {
        self.enclose_in_chars(start, end, None, Some('\\'))
    }

    /// Wrap a string in a pair of characters, with the closing character matching the first character
    /// if it a parenthesis (round bracket), angle bracket, (square)  bracket or curly brace. Otherwise
    /// the closing character will be the same opening character
    /// The optional escape character is inserted before occurrences of the end character
    /// unless the preceding character is the escape character itself to avoid double escaping of pre-escaped strings
    fn wrap_escaped(&self, opening: char, escape_char: Option<char>) -> String {
        let end = match_closing_char(opening);
        self.enclose_in_chars(opening, end, None, escape_char)
    }

    /// wrap a string in the same opening and closing character
    fn wrap(&self, opening: char) -> String {
        let end = match_closing_char(opening);
        self.enclose_in_chars(opening, end, None, None)
    }

    /// Wrap in matching characters escaped by a backslash \
    fn wrap_safe(&self, opening: char) -> String {
        let end = match_closing_char(opening);
        self.enclose_in_chars(opening, end, None, Some('\\'))
    }

    /// wrap in parentheses (round brackets) with an optional prefix before the main content
    fn in_parentheses(&self, prefix: Option<&str>) -> String {
        self.enclose_in_chars('(', ')', prefix, None)
    }

    /// wrap in parentheses (round brackets)
    fn parenthesize(&self) -> String {
        self.wrap('(')
    }

    /// wrap in parentheses (round brackets)
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
        self.wrap_escaped('"', Some('\\'))
    }

    /// Wrap in single quotes with escaped quotes in the content
    fn single_quotes_safe(&self) -> String {
        self.wrap_escaped('\'', Some('\\'))
    }
}

impl<T: AsRef<str>> SimpleEnclose for T {
    fn enclose_in_chars(
        &self,
        start: char,
        end: char,
        prefix: Option<&str>,
        escape_char: Option<char>,
    ) -> String {
        let s = self.as_ref();
        let mut out = match escape_char {
            Some(esc_char) => {
                if s.contains(end) {
                    escape_in_str(s.chars(), end, esc_char)
                } else {
                    s.to_owned()
                }
            }
            _ => s.to_owned(),
        };
        out.insert(0, start);
        if let Some(pre) = prefix {
            out.insert_str(1, pre);
        }
        out.push(end);
        out
    }
}

/// Escape occurrences of `end` within a string using `esc_char`.
/// Already-escaped instances are detected and left alone:
/// - When `esc_char != end`: an odd number of consecutive escape chars before `end` means it's already escaped.
/// - When `esc_char == end` (CSV-style doubling): adjacent pairs are treated as already-escaped.
pub fn escape_in_str(chars: Chars, end: char, esc_char: char) -> String {
    let char_vec: Vec<char> = chars.collect();
    let mut out = String::with_capacity(char_vec.len() + 8);
    if esc_char == end {
        let mut i = 0;
        while i < char_vec.len() {
            if char_vec[i] == end {
                out.push(end);
                out.push(end);
                if i + 1 < char_vec.len() && char_vec[i + 1] == end {
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                out.push(char_vec[i]);
                i += 1;
            }
        }
    } else {
        for (i, &ch) in char_vec.iter().enumerate() {
            if ch == end {
                let mut esc_count = 0;
                let mut j = i;
                while j > 0 && char_vec[j - 1] == esc_char {
                    esc_count += 1;
                    j -= 1;
                }
                if esc_count % 2 == 0 {
                    out.push(esc_char);
                }
            }
            out.push(ch);
        }
    }
    out
}

// Private Helper functions
fn match_closing_char(opening: char) -> char {
    match opening {
        '(' => ')',
        '<' => '>',
        '{' => '}',
        '[' => ']',
        '\u{2018}' => '\u{2019}', // ' '
        '\u{201C}' => '\u{201D}', // " "
        '\u{201E}' => '\u{201C}', // „ "
        '\u{201F}' => '\u{201E}', // ‟ „
        '\u{2039}' => '\u{203A}', // ‹ ›
        '\u{00AB}' => '\u{00BB}', // « »
        '\u{275B}' => '\u{275C}', // ❛ ❜
        '\u{275D}' => '\u{275E}', // ❝ ❞
        '\u{2E42}' => '\u{201D}', // ⹂ "
        '\u{300C}' => '\u{300D}', // 「 」
        '\u{300E}' => '\u{300F}', // 『 』
        '\u{FE41}' => '\u{FE42}', // ﹁ ﹂
        '\u{FE43}' => '\u{FE44}', // ﹃ ﹄
        _ => opening,
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
            sample_str_3.wrap_escaped('"', Some('"')),
            expected_quoted_str_3
        );
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
            input.wrap_escaped('"', Some('"')),
            r#""She said ""hello""""#
        );
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

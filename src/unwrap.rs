use crate::delimiter::Delimiter;
use crate::escape::EscapeStyle;
use crate::match_closing_char;
use crate::scan::{Nesting, ScanOptions};
use std::borrow::Cow;

/// One piece of a segmented string: either the content of an enclosure, or the
/// plain text outside the enclosures, delimiters excluded.
///
/// `Enclosure` is a `Cow` because escape characters are dropped from the
/// content, so an escaped enclosure is not a contiguous slice of the source.
/// Where nothing was unescaped — the common case — it borrows and costs no
/// allocation. `Outside` is never rewritten, so it always borrows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapturedSegment<'a> {
    Enclosure(Cow<'a, str>),
    Outside(&'a str),
}

impl<'a> CapturedSegment<'a> {
    /// The text of this segment, whichever kind it is.
    pub fn as_str(&self) -> &str {
        match self {
            CapturedSegment::Enclosure(text) => text.as_ref(),
            CapturedSegment::Outside(text) => text,
        }
    }

    pub fn is_enclosure(&self) -> bool {
        matches!(self, CapturedSegment::Enclosure(_))
    }

    pub fn is_outside(&self) -> bool {
        matches!(self, CapturedSegment::Outside(_))
    }

    /// The text if this is an enclosure, otherwise `None`.
    pub fn enclosure(&self) -> Option<&str> {
        match self {
            CapturedSegment::Enclosure(text) => Some(text.as_ref()),
            CapturedSegment::Outside(_) => None,
        }
    }
}

/// Find the next opening position in `hay`, returning the byte index where the
/// opening delimiter starts and the byte index of the first content character.
fn next_opening<S: Delimiter>(hay: &str, start: &S) -> Option<(usize, usize)> {
    start
        .find_from(hay, 0)
        .map(|at| (at, at + start.match_len(hay, at).unwrap_or(0)))
}

/// Read enclosure content from the start of `rest`, returning the content and
/// the byte index just past the closing delimiter. `None` if unterminated.
///
/// The output string is only allocated once a character is actually dropped,
/// so unescaped content is returned as a borrowed slice.
fn scan_content<'a, S: Delimiter, E: Delimiter>(
    rest: &'a str,
    start: &S,
    end: &E,
    options: ScanOptions<'_>,
) -> Option<(Cow<'a, str>, usize)> {
    let escape = options.escape;
    let mut rewritten: Option<String> = None;
    let mut index = 0;
    let mut depth = 0usize;
    // The quote character currently open, if any. Everything up to its partner
    // is content, including closing delimiters.
    let mut in_quote: Option<char> = None;

    while index < rest.len() {
        if let Some(quote_char) = in_quote {
            let character = rest[index..].chars().next().expect("index on a boundary");
            // An escaped character inside a quoted region is copied whole, so
            // an escaped quote does not close the region.
            if let EscapeStyle::Char(escape_char) = escape {
                if character == escape_char {
                    if let Some(next) = rest[index + character.len_utf8()..].chars().next() {
                        if let Some(buffer) = rewritten.as_mut() {
                            buffer.push(character);
                            buffer.push(next);
                        }
                        index += character.len_utf8() + next.len_utf8();
                        continue;
                    }
                }
            }
            if character == quote_char {
                in_quote = None;
            }
            if let Some(buffer) = rewritten.as_mut() {
                buffer.push(character);
            }
            index += character.len_utf8();
            continue;
        }

        if let Some(quote_char) = rest[index..]
            .chars()
            .next()
            .filter(|c| options.quotes.contains(c))
        {
            in_quote = Some(quote_char);
            if let Some(buffer) = rewritten.as_mut() {
                buffer.push(quote_char);
            }
            index += quote_char.len_utf8();
            continue;
        }

        if let Nesting::Pair(_, close) = options.nesting {
            if depth > 0 {
                if let Some(width) = close.match_len(rest, index) {
                    depth -= 1;
                    rewritten
                        .get_or_insert_with(|| rest[..index].to_string())
                        .push_str(&rest[index..index + width]);
                    index += width;
                    continue;
                }
            }
        }

        if let Some(width) = end.match_len(rest, index) {
            // A doubled closing delimiter is one literal; a single one closes.
            // Char(esc) where esc is the closing delimiter means the same
            // thing, and must, because escaping on the enclose side produces
            // exactly that doubled pair.
            let doubles = escape == EscapeStyle::Doubled
                || matches!(escape, EscapeStyle::Char(esc)
                    if esc.len_utf8() == width && rest[index..].starts_with(esc));
            if doubles && end.match_len(rest, index + width).is_some() {
                rewritten
                    .get_or_insert_with(|| rest[..index].to_string())
                    .push_str(&rest[index..index + width]);
                index += width * 2;
                continue;
            }
            if depth > 0 {
                depth -= 1;
                rewritten
                    .get_or_insert_with(|| rest[..index].to_string())
                    .push_str(&rest[index..index + width]);
                index += width;
                continue;
            }
            // An escaped closing delimiter is consumed by the escape branch
            // below before the scan ever reaches it here, so a match at this
            // point always closes the content.
            return Some((finish(rewritten, rest, index), index + width));
        }

        // Counting an opening is only meaningful where it differs from the
        // closing; identical ones give no way to tell them apart.
        let opener = match options.nesting {
            Nesting::None => None,
            Nesting::Delimiters => start
                .match_len(rest, index)
                .filter(|_| end.match_len(rest, index).is_none()),
            Nesting::Pair(open, close) => open
                .match_len(rest, index)
                .filter(|_| close.match_len(rest, index).is_none()),
        };
        if let Some(width) = opener {
            depth += 1;
            rewritten
                .get_or_insert_with(|| rest[..index].to_string())
                .push_str(&rest[index..index + width]);
            index += width;
            continue;
        }

        if let EscapeStyle::Char(escape_char) = escape {
            if rest[index..].starts_with(escape_char) {
                let after = index + escape_char.len_utf8();
                // A start or end delimiter preceded by the escape character is
                // content; the escape itself is not. Any other escaped
                // character is left exactly as it was found.
                match start
                    .match_len(rest, after)
                    .or_else(|| end.match_len(rest, after))
                {
                    Some(width) => {
                        rewritten
                            .get_or_insert_with(|| rest[..index].to_string())
                            .push_str(&rest[after..after + width]);
                        index = after + width;
                    }
                    None => {
                        if let Some(buffer) = rewritten.as_mut() {
                            buffer.push(escape_char);
                        }
                        index = after;
                    }
                }
                continue;
            }
        }

        let character = rest[index..].chars().next().expect("index on a boundary");
        if let Some(buffer) = rewritten.as_mut() {
            buffer.push(character);
        }
        index += character.len_utf8();
    }
    None
}

fn finish(rewritten: Option<String>, rest: &str, upto: usize) -> Cow<'_, str> {
    match rewritten {
        Some(buffer) => Cow::Owned(buffer),
        None => Cow::Borrowed(&rest[..upto]),
    }
}

/// Trait with extension methods to extract the content wrapped in bounding
/// delimiters. This is the inverse of `SimpleEnclose`.
pub trait SimpleExtract: AsRef<str> {
    /// Split the whole string into alternating enclosures and the plain text
    /// around them, in source order.
    ///
    /// ```
    /// # use enclose_strings::{SimpleExtract, CapturedSegment, EscapeStyle};
    /// let s = "prefix!(contents_1)inbetween!(contents_2)suffix";
    /// let segments = s.extract_enclosures("!(", ')', EscapeStyle::Char('\\'));
    /// assert_eq!(segments[0], CapturedSegment::Outside("prefix"));
    /// assert_eq!(segments[1].enclosure(), Some("contents_1"));
    /// ```
    ///
    /// Any marker forms part of the opening delimiter: `"!("` and `"(?<="`
    /// both work as a `start`, whether the marker sits before or after the
    /// bracket.
    ///
    /// Empty `Outside` segments are omitted, so adjacent enclosures produce no
    /// empty segment between them. An unterminated enclosure, and everything
    /// after it, is reported as a single trailing `Outside`.
    fn extract_enclosures_with<S: Delimiter, E: Delimiter>(
        &self,
        start: S,
        end: E,
        options: ScanOptions<'_>,
    ) -> Vec<CapturedSegment<'_>>;

    /// As [`SimpleExtract::extract_enclosures_with`], with escaping as the only
    /// rule. Quoted regions and nesting are not considered.
    fn extract_enclosures<S: Delimiter, E: Delimiter>(
        &self,
        start: S,
        end: E,
        escape: EscapeStyle,
    ) -> Vec<CapturedSegment<'_>> {
        self.extract_enclosures_with(start, end, ScanOptions::escaped(escape))
    }

    /// The first enclosure under the given scan rules.
    fn extract_first_enclosure_with<S: Delimiter, E: Delimiter>(
        &self,
        start: S,
        end: E,
        options: ScanOptions<'_>,
    ) -> Option<String> {
        self.extract_enclosures_with(start, end, options)
            .into_iter()
            .find_map(|segment| match segment {
                CapturedSegment::Enclosure(content) => Some(content.into_owned()),
                CapturedSegment::Outside(_) => None,
            })
    }

    /// The first balanced enclosure bounded by `opening` and its matching
    /// closing character, treating quoted regions as opaque and tracking
    /// nesting.
    ///
    /// ```
    /// # use enclose_strings::SimpleExtract;
    /// let call = r#"add_title("Latest Stats (2024-2025)", "en-GB")"#;
    /// assert_eq!(
    ///     call.extract_balanced('('),
    ///     Some(r#""Latest Stats (2024-2025)", "en-GB""#.to_string())
    /// );
    /// ```
    fn extract_balanced(&self, opening: char) -> Option<String> {
        self.extract_nth_balanced(opening, 0)
    }

    /// Backward-compatible alias for [`SimpleExtract::extract_balanced`].
    fn extract_arguments(&self, opening: char) -> Option<String> {
        self.extract_balanced(opening)
    }

    /// The *n*th balanced enclosure (0-indexed) bounded by `opening` and its
    /// matching closing character, treating quoted regions as opaque and
    /// tracking nesting.
    fn extract_nth_balanced(&self, opening: char, n: usize) -> Option<String> {
        self.extract_captures(opening)
            .into_iter()
            .filter_map(|seg| match seg {
                CapturedSegment::Enclosure(content) => Some(content.into_owned()),
                CapturedSegment::Outside(_) => None,
            })
            .nth(n)
    }

    /// Every balanced enclosure content bounded by `opening` and its matching
    /// closing character, treating quoted regions as opaque and tracking
    /// nesting. Returns only the content strings, discarding the text
    /// between enclosures.
    fn extract_all_balanced(&self, opening: char) -> Vec<String> {
        self.extract_captures(opening)
            .into_iter()
            .filter_map(|seg| match seg {
                CapturedSegment::Enclosure(content) => Some(content.into_owned()),
                CapturedSegment::Outside(_) => None,
            })
            .collect()
    }

    /// Every enclosure bounded by `opening` and its matching closing
    /// character, with the text around them, using backslash escaping.
    ///
    /// The terse form of [`SimpleExtract::extract_enclosures`] for the common
    /// case: no quoted regions and no nesting.
    fn extract_segments(&self, opening: char) -> Vec<CapturedSegment<'_>> {
        let closing = match_closing_char(opening);
        self.extract_enclosures_with(
            opening,
            closing,
            ScanOptions::escaped(EscapeStyle::Char('\\')),
        )
    }

    /// Split the string into alternating enclosures and outside text,
    /// treating quoted regions as opaque and tracking nesting.
    fn extract_captures(&self, opening: char) -> Vec<CapturedSegment<'_>> {
        let closing = match_closing_char(opening);
        self.extract_enclosures_with(opening, closing, ScanOptions::quoted())
    }

    /// Backward-compatible alias for [`SimpleExtract::extract_captures`].
    fn extract_quoted_segments(&self, opening: char) -> Vec<CapturedSegment<'_>> {
        self.extract_captures(opening)
    }

    /// The first enclosure, ignoring the text around it.
    ///
    /// `start` and `end` may each be a `char`, a `&str` or a `String`, so an
    /// opening marker is part of the delimiter rather than a separate argument.
    fn extract_first_enclosure<S: Delimiter, E: Delimiter>(
        &self,
        start: S,
        end: E,
        escape: EscapeStyle,
    ) -> Option<String> {
        self.extract_enclosures(start, end, escape)
            .into_iter()
            .find_map(|segment| match segment {
                CapturedSegment::Enclosure(content) => Some(content.into_owned()),
                CapturedSegment::Outside(_) => None,
            })
    }

    /// Every balanced enclosure in the string at every nesting depth,
    /// ordered by opening position (outermost groups appear first when
    /// they start before their children).
    ///
    /// When the opening and closing characters differ, nesting is tracked
    /// so `((a or b) and (c or d))` yields all three groups rather than
    /// stopping at the first `)`.  When they are identical (e.g. `"`),
    /// nesting is impossible and the method falls back to a flat scan.
    fn extract_all_enclosed(&self, opening: char) -> Vec<String> {
        let closing = match_closing_char(opening);
        if opening == closing {
            return self
                .extract_enclosures(opening, closing, EscapeStyle::Char('\\'))
                .iter()
                .filter_map(|segment| segment.enclosure().map(|text| text.to_string()))
                .collect();
        }
        let s = self.as_ref();
        let mut results: Vec<(usize, String)> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        for (idx, ch) in s.char_indices() {
            if ch == opening {
                stack.push(idx + ch.len_utf8());
            } else if ch == closing && !stack.is_empty() {
                let content_start = stack.pop().unwrap();
                results.push((content_start, s[content_start..idx].to_string()));
            }
        }
        results.sort_by_key(|(pos, _)| *pos);
        results.into_iter().map(|(_, content)| content).collect()
    }

    /// Extract the first enclosure, treating the content literally
    fn extract_first<S: Delimiter, E: Delimiter>(&self, start: S, end: E) -> Option<String> {
        self.extract_first_enclosure(start, end, EscapeStyle::None)
    }

    /// Extract the first enclosure bounded by `opening` and its matching
    /// closing character, with backslash escaping
    fn extract_enclosed(&self, opening: char) -> Option<String> {
        let closing = match_closing_char(opening);
        self.extract_first_enclosure(opening, closing, EscapeStyle::Char('\\'))
    }

    /// Extract the first enclosure bounded by `opening` and its matching
    /// closing character, using the doubling convention
    fn extract_enclosed_doubled(&self, opening: char) -> Option<String> {
        let closing = match_closing_char(opening);
        self.extract_first_enclosure(opening, closing, EscapeStyle::Doubled)
    }

    /// The first group in parentheses, `( )` — round brackets.
    fn extract_from_parentheses(&self) -> Option<String> {
        self.extract_enclosed('(')
    }

    /// The first group in brackets, `[ ]` — square brackets.
    fn extract_from_brackets(&self) -> Option<String> {
        self.extract_enclosed('[')
    }

    /// The first group in braces, `{ }` — curly brackets.
    fn extract_from_braces(&self) -> Option<String> {
        self.extract_enclosed('{')
    }

    /// The first group in double quotes, `" "`.
    fn extract_from_double_quotes(&self) -> Option<String> {
        self.extract_enclosed('"')
    }

    /// The first group in single quotes, `' '`. Note that an apostrophe is
    /// the same code point, so contractions open a group.
    fn extract_from_single_quotes(&self) -> Option<String> {
        self.extract_enclosed('\'')
    }

    /// All captures from parentheses `( )`, with nesting and quoted regions.
    fn extract_all_from_parentheses(&self) -> Vec<CapturedSegment<'_>> {
        self.extract_captures('(')
    }

    /// All captures from brackets `[ ]`, with nesting and quoted regions.
    fn extract_all_from_brackets(&self) -> Vec<CapturedSegment<'_>> {
        self.extract_captures('[')
    }

    /// All captures from braces `{ }`, with nesting and quoted regions.
    fn extract_all_from_braces(&self) -> Vec<CapturedSegment<'_>> {
        self.extract_captures('{')
    }
}

impl<T: AsRef<str>> SimpleExtract for T {
    fn extract_enclosures_with<S: Delimiter, E: Delimiter>(
        &self,
        start: S,
        end: E,
        options: ScanOptions<'_>,
    ) -> Vec<CapturedSegment<'_>> {
        let source = self.as_ref();
        let mut segments = Vec::new();
        let mut cursor = 0;

        while cursor < source.len() {
            let Some((opens_at, content_at)) = next_opening(&source[cursor..], &start) else {
                break;
            };
            let opens_at = cursor + opens_at;
            let content_at = cursor + content_at;

            match scan_content(&source[content_at..], &start, &end, options) {
                Some((content, consumed)) => {
                    if opens_at > cursor {
                        segments.push(CapturedSegment::Outside(&source[cursor..opens_at]));
                    }
                    segments.push(CapturedSegment::Enclosure(content));
                    cursor = content_at + consumed;
                }
                // Opened but never closed: the rest of the string is plain text.
                None => break,
            }
        }
        if cursor < source.len() {
            segments.push(CapturedSegment::Outside(&source[cursor..]));
        }
        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleEnclose;
    use std::borrow::Cow;

    #[test]
    fn test_extract_from_chars() {
        let s = "...prefix(content)suffix";
        assert_eq!(
            s.extract_first_enclosure("prefix(", ')', EscapeStyle::Char('\\')),
            Some("content".to_string())
        );
    }

    #[test]
    fn test_extract_without_prefix() {
        assert_eq!(
            "(content)".extract_first_enclosure('(', ')', EscapeStyle::None),
            Some("content".to_string())
        );
        assert_eq!(
            r#"say "hello" now"#.extract_first_enclosure('"', '"', EscapeStyle::None),
            Some("hello".to_string())
        );
    }

    #[test]
    fn test_escaped_end_char_is_content() {
        // \) does not close the content, and the escape is dropped
        assert_eq!(
            r#"(a (nested\) call)"#.extract_first_enclosure('(', ')', EscapeStyle::Char('\\')),
            Some("a (nested) call".to_string())
        );
        // without an escape char the same input stops at the first )
        assert_eq!(
            r#"(a (nested\) call)"#.extract_first_enclosure('(', ')', EscapeStyle::None),
            Some(r#"a (nested\"#.to_string())
        );
    }

    #[test]
    fn test_unrelated_escape_sequence_preserved() {
        // \t is not an escaped bounding char, so the backslash stays
        assert_eq!(
            r#""path\to\file""#.extract_first_enclosure('"', '"', EscapeStyle::Char('\\')),
            Some(r#"path\to\file"#.to_string())
        );
    }

    #[test]
    fn test_csv_style_doubling() {
        assert_eq!(
            r#""She said ""hello""""#.extract_first_enclosure('"', '"', EscapeStyle::Doubled),
            Some(r#"She said "hello""#.to_string())
        );
    }

    #[test]
    fn test_missing_prefix_or_terminator() {
        // prefix absent
        assert_eq!(
            "...other(content)".extract_first_enclosure("prefix(", ')', EscapeStyle::None),
            None
        );
        // opening char never reached
        assert_eq!(
            "prefix no brackets".extract_first_enclosure("prefix(", ')', EscapeStyle::None),
            None
        );
        // unterminated
        assert_eq!(
            "(content".extract_first_enclosure('(', ')', EscapeStyle::None),
            None
        );
    }

    #[test]
    fn test_prefix_only_matches_after_itself() {
        // the ( before the prefix must be ignored
        assert_eq!(
            "(early)prefix(wanted)".extract_first_enclosure("prefix(", ')', EscapeStyle::None),
            Some("wanted".to_string())
        );
    }

    #[test]
    fn test_multibyte_before_prefix() {
        // é is two bytes: a byte offset here would land mid-content
        assert_eq!(
            "café prefix(content)".extract_first_enclosure("prefix(", ')', EscapeStyle::None),
            Some("content".to_string())
        );
    }

    #[test]
    fn test_round_trip_with_enclose() {
        let original = r#"a (nested) call"#;
        let wrapped = original.wrap_safe('(');
        assert_eq!(wrapped, r#"(a (nested\) call)"#);
        assert_eq!(
            wrapped.extract_first_enclosure('(', ')', EscapeStyle::Char('\\')),
            Some(original.to_string())
        );

        let quoted = r#"She said "hello""#.wrap_escaped('"', EscapeStyle::Doubled);
        assert_eq!(
            quoted.extract_first_enclosure('"', '"', EscapeStyle::Doubled),
            Some(r#"She said "hello""#.to_string())
        );
    }

    // --- extract_first --------------------------------------------------

    #[test]
    fn test_extract_first() {
        assert_eq!(
            "(content)".extract_first('(', ')'),
            Some("content".to_string())
        );
        // the opening character need not be at the start
        assert_eq!(
            "call(arg);".extract_first('(', ')'),
            Some("arg".to_string())
        );
        // identical bounding characters
        assert_eq!(
            r#"say "hello" now"#.extract_first('"', '"'),
            Some("hello".to_string())
        );
        // empty content is still a match, distinct from None
        assert_eq!("()".extract_first('(', ')'), Some(String::new()));
    }

    #[test]
    fn test_extract_first_stops_at_first_end() {
        // no escape handling: the first closing character wins, even an escaped one
        assert_eq!(
            r#"(a\) b)"#.extract_first('(', ')'),
            Some(r#"a\"#.to_string())
        );
        // and nesting is not tracked
        assert_eq!(
            "(outer (inner) rest)".extract_first('(', ')'),
            Some("outer (inner".to_string())
        );
    }

    #[test]
    fn test_extract_first_none_cases() {
        assert_eq!("no brackets here".extract_first('(', ')'), None);
        assert_eq!("(unterminated".extract_first('(', ')'), None);
        // a closing character with no opening one
        assert_eq!("closed) only".extract_first('(', ')'), None);
    }

    // --- extract_escaped ------------------------------------------------

    #[test]
    fn test_extract_escaped() {
        // an escaped closing character is content, and the escape is dropped
        assert_eq!(
            r#"(a (nested\) call)"#.extract_first_enclosure('(', ')', EscapeStyle::Char('\\')),
            Some("a (nested) call".to_string())
        );
        assert_eq!(
            r#""say \"hi\" now""#.extract_first_enclosure('"', '"', EscapeStyle::Char('\\')),
            Some(r#"say "hi" now"#.to_string())
        );
    }

    #[test]
    fn test_extract_escaped_without_escape_char() {
        // None makes it equivalent to extract_first
        let sample = r#"(a\) b)"#;
        assert_eq!(
            sample.extract_first_enclosure('(', ')', EscapeStyle::None),
            sample.extract_first('(', ')')
        );
    }

    #[test]
    fn test_extract_escaped_csv_doubling() {
        // escape char == end char: a doubled pair is one literal
        assert_eq!(
            r#""She said ""hello""""#.extract_first_enclosure('"', '"', EscapeStyle::Doubled),
            Some(r#"She said "hello""#.to_string())
        );
        // a single end char still closes
        assert_eq!(
            r#""plain" tail"#.extract_first_enclosure('"', '"', EscapeStyle::Doubled),
            Some("plain".to_string())
        );
    }

    #[test]
    fn test_extract_escaped_round_trip() {
        let original = r#"contains ) and ( both"#;
        let wrapped = original.enclose_escaped('(', ')', EscapeStyle::Char('\\'));
        assert_eq!(
            wrapped.extract_first_enclosure('(', ')', EscapeStyle::Char('\\')),
            Some(original.to_string())
        );
    }

    // --- extract_enclosed -----------------------------------------------

    #[test]
    fn test_extract_enclosed_bracket_pairs() {
        // the closing character is derived from the opening one
        assert_eq!(
            "(content)".extract_enclosed('('),
            Some("content".to_string())
        );
        assert_eq!(
            "[content]".extract_enclosed('['),
            Some("content".to_string())
        );
        assert_eq!(
            "{content}".extract_enclosed('{'),
            Some("content".to_string())
        );
        assert_eq!(
            "<content>".extract_enclosed('<'),
            Some("content".to_string())
        );
    }

    #[test]
    fn test_extract_enclosed_same_char_pairs() {
        // characters with no distinct closing form close on themselves
        assert_eq!(
            r#""content""#.extract_enclosed('"'),
            Some("content".to_string())
        );
        assert_eq!(
            "'content'".extract_enclosed('\''),
            Some("content".to_string())
        );
        // including an arbitrary character
        assert_eq!(
            "xcontentx".extract_enclosed('x'),
            Some("content".to_string())
        );
    }

    #[test]
    fn test_extract_enclosed_unicode_pairs() {
        assert_eq!(
            "\u{201C}content\u{201D}".extract_enclosed('\u{201C}'),
            Some("content".to_string())
        );
        assert_eq!(
            "\u{2018}content\u{2019}".extract_enclosed('\u{2018}'),
            Some("content".to_string())
        );
        assert_eq!(
            "\u{00AB}content\u{00BB}".extract_enclosed('\u{00AB}'),
            Some("content".to_string())
        );
        assert_eq!(
            "\u{300C}content\u{300D}".extract_enclosed('\u{300C}'),
            Some("content".to_string())
        );
    }

    #[test]
    fn test_extract_enclosed_uses_backslash_escaping() {
        // backslash is implied, so an escaped closing character is content
        assert_eq!(
            r#"(a (nested\) call)"#.extract_enclosed('('),
            Some("a (nested) call".to_string())
        );
        assert_eq!(
            r#""say \"hi\"""#.extract_enclosed('"'),
            Some(r#"say "hi""#.to_string())
        );
    }

    #[test]
    fn test_extract_enclosed_none_cases() {
        assert_eq!("no brackets".extract_enclosed('('), None);
        assert_eq!("(unterminated".extract_enclosed('('), None);
        // mismatched: the derived closing character for [ is ], not )
        assert_eq!("[content)".extract_enclosed('['), None);
    }

    #[test]
    fn test_extract_enclosed_round_trip_with_wrap_safe() {
        // wrap_safe is the exact inverse: same pairing, same escape character
        for opening in ['(', '[', '{', '<', '"', '\'', '\u{201C}', '\u{00AB}'] {
            let original = "plain content";
            let wrapped = original.wrap_safe(opening);
            assert_eq!(
                wrapped.extract_enclosed(opening),
                Some(original.to_string()),
                "round trip failed for opening char {:?}",
                opening
            );
        }
    }

    // --- extract_enclosures -----------------------------------------------

    #[test]
    fn test_extract_enclosures_with_marker() {
        let sample_str = "prefix!(contents_1)inbetween!(contents_2)suffix";
        assert_eq!(
            sample_str.extract_enclosures("!(", ')', EscapeStyle::Char('\\')),
            vec![
                CapturedSegment::Outside("prefix"),
                CapturedSegment::Enclosure(Cow::Borrowed("contents_1")),
                CapturedSegment::Outside("inbetween"),
                CapturedSegment::Enclosure(Cow::Borrowed("contents_2")),
                CapturedSegment::Outside("suffix"),
            ]
        );
    }

    #[test]
    fn test_extract_enclosures_without_marker() {
        assert_eq!(
            "a(one)b(two)c".extract_enclosures('(', ')', EscapeStyle::None),
            vec![
                CapturedSegment::Outside("a"),
                CapturedSegment::Enclosure(Cow::Borrowed("one")),
                CapturedSegment::Outside("b"),
                CapturedSegment::Enclosure(Cow::Borrowed("two")),
                CapturedSegment::Outside("c"),
            ]
        );
    }

    #[test]
    fn test_extract_enclosures_omits_empty_outside_segments() {
        assert_eq!(
            "(one)(two)".extract_enclosures('(', ')', EscapeStyle::None),
            vec![
                CapturedSegment::Enclosure(Cow::Borrowed("one")),
                CapturedSegment::Enclosure(Cow::Borrowed("two")),
            ]
        );
        // an empty enclosure is still reported, unlike an empty Outside segment
        assert_eq!(
            "()".extract_enclosures('(', ')', EscapeStyle::None),
            vec![CapturedSegment::Enclosure(Cow::Borrowed(""))]
        );
    }

    #[test]
    fn test_extract_enclosures_marker_not_before_opening_is_text() {
        assert_eq!(
            "hi! there !(inside) end".extract_enclosures("!(", ')', EscapeStyle::None),
            vec![
                CapturedSegment::Outside("hi! there "),
                CapturedSegment::Enclosure(Cow::Borrowed("inside")),
                CapturedSegment::Outside(" end"),
            ]
        );
        assert_eq!(
            "(unmarked) only".extract_enclosures("!(", ')', EscapeStyle::None),
            vec![CapturedSegment::Outside("(unmarked) only")]
        );
    }

    #[test]
    fn test_extract_enclosures_escaping() {
        assert_eq!(
            r#"a(b\) c)d"#.extract_enclosures('(', ')', EscapeStyle::Char('\\')),
            vec![
                CapturedSegment::Outside("a"),
                CapturedSegment::Enclosure(Cow::Owned("b) c".to_string())),
                CapturedSegment::Outside("d"),
            ]
        );
        assert_eq!(
            r#""a ""x"" b","plain""#.extract_enclosures('"', '"', EscapeStyle::Doubled),
            vec![
                CapturedSegment::Enclosure(Cow::Owned(r#"a "x" b"#.to_string())),
                CapturedSegment::Outside(","),
                CapturedSegment::Enclosure(Cow::Borrowed("plain")),
            ]
        );
    }

    #[test]
    fn test_extract_enclosures_borrows_when_nothing_unescaped() {
        let segments = "x(plain)y".extract_enclosures('(', ')', EscapeStyle::Char('\\'));
        match &segments[1] {
            CapturedSegment::Enclosure(content) => assert!(
                matches!(content, Cow::Borrowed(_)),
                "should borrow, got {content:?}"
            ),
            other => panic!("expected an enclosure, got {other:?}"),
        }
    }

    #[test]
    fn test_extract_enclosures_unterminated_and_absent() {
        assert_eq!(
            "before (never closed".extract_enclosures('(', ')', EscapeStyle::None),
            vec![CapturedSegment::Outside("before (never closed")]
        );
        assert_eq!(
            "no brackets".extract_enclosures('(', ')', EscapeStyle::None),
            vec![CapturedSegment::Outside("no brackets")]
        );
        assert_eq!("".extract_enclosures('(', ')', EscapeStyle::None), vec![]);
    }

    #[test]
    fn test_extract_enclosures_multibyte() {
        assert_eq!(
            "café(contenu)thé".extract_enclosures('(', ')', EscapeStyle::None),
            vec![
                CapturedSegment::Outside("café"),
                CapturedSegment::Enclosure(Cow::Borrowed("contenu")),
                CapturedSegment::Outside("thé"),
            ]
        );
        assert_eq!(
            "\u{201C}one\u{201D}mid\u{201C}two\u{201D}".extract_enclosures(
                '\u{201C}',
                '\u{201D}',
                EscapeStyle::None
            ),
            vec![
                CapturedSegment::Enclosure(Cow::Borrowed("one")),
                CapturedSegment::Outside("mid"),
                CapturedSegment::Enclosure(Cow::Borrowed("two")),
            ]
        );
    }

    #[test]
    fn test_extract_all_enclosed() {
        // flat (non-nested) case is unchanged
        assert_eq!(
            "a(one)b(two)c".extract_all_enclosed('('),
            vec!["one".to_string(), "two".to_string()]
        );
        assert_eq!(
            "nothing here".extract_all_enclosed('('),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_extract_all_enclosed_nested_parentheses() {
        assert_eq!(
            "((a or b) and (c or d))".extract_all_enclosed('('),
            vec![
                "(a or b) and (c or d)".to_string(),
                "a or b".to_string(),
                "c or d".to_string(),
            ]
        );
    }

    #[test]
    fn test_extract_all_enclosed_nested_brackets() {
        assert_eq!(
            "[[a or b] and [c or d]]".extract_all_enclosed('['),
            vec![
                "[a or b] and [c or d]".to_string(),
                "a or b".to_string(),
                "c or d".to_string(),
            ]
        );
    }

    #[test]
    fn test_extract_all_enclosed_deeply_nested() {
        assert_eq!(
            "(a(b(c)))".extract_all_enclosed('('),
            vec![
                "a(b(c))".to_string(),
                "b(c)".to_string(),
                "c".to_string(),
            ]
        );
    }

    #[test]
    fn test_extract_all_enclosed_mixed_depths() {
        // two top-level groups, one of which has nesting
        assert_eq!(
            "(a)(b(c)d)".extract_all_enclosed('('),
            vec![
                "a".to_string(),
                "b(c)d".to_string(),
                "c".to_string(),
            ]
        );
    }

    #[test]
    fn test_extract_all_enclosed_identical_delimiters_flat() {
        // quotes can't nest, so the flat scan is used
        assert_eq!(
            r#""one" and "two""#.extract_all_enclosed('"'),
            vec!["one".to_string(), "two".to_string()]
        );
    }

    #[test]
    fn test_extract_all_enclosed_unmatched_ignored() {
        // unmatched closing brackets are skipped
        assert_eq!(
            ")extra(content)".extract_all_enclosed('('),
            vec!["content".to_string()]
        );
        // unclosed opening brackets produce nothing for that group
        assert_eq!(
            "(closed)(unclosed".extract_all_enclosed('('),
            vec!["closed".to_string()]
        );
    }

    // --- terse segment variants -------------------------------------------

    #[test]
    fn test_extract_segments_defaults() {
        // matched closing character and backslash escaping, no other arguments
        assert_eq!(
            "a(one)b(two)c".extract_segments('('),
            "a(one)b(two)c".extract_enclosures('(', ')', EscapeStyle::Char('\\'))
        );
        assert_eq!(
            "x[one]y".extract_segments('['),
            vec![
                CapturedSegment::Outside("x"),
                CapturedSegment::Enclosure(Cow::Borrowed("one")),
                CapturedSegment::Outside("y"),
            ]
        );
        // the escaping default is applied, not merely accepted
        assert_eq!(
            r#"(a\) b)"#.extract_segments('('),
            vec![CapturedSegment::Enclosure(Cow::Owned("a) b".to_string()))]
        );
    }

    #[test]
    fn test_extract_captures() {
        let call = r#"add_title("Latest Stats (2024-2025)", "en-GB")"#;
        assert_eq!(
            call.extract_captures('('),
            vec![
                CapturedSegment::Outside("add_title"),
                CapturedSegment::Enclosure(Cow::Borrowed(r#""Latest Stats (2024-2025)", "en-GB""#)),
            ]
        );
        // nesting is tracked as well as quoting
        assert_eq!(
            "f(g(x), y)".extract_captures('('),
            vec![
                CapturedSegment::Outside("f"),
                CapturedSegment::Enclosure(Cow::Owned("g(x), y".to_string())),
            ]
        );
        // backward-compat alias agrees
        assert_eq!(
            "f(g(x), y)".extract_quoted_segments('('),
            "f(g(x), y)".extract_captures('('),
        );
    }

    #[test]
    fn test_extract_balanced() {
        let call = r#"add_title("Latest Stats (2024-2025)", "en-GB")"#;
        assert_eq!(
            call.extract_balanced('('),
            Some(r#""Latest Stats (2024-2025)", "en-GB""#.to_string())
        );
        // backward-compat alias agrees
        assert_eq!(
            call.extract_arguments('('),
            call.extract_balanced('('),
        );
    }

    #[test]
    fn test_extract_nth_balanced() {
        let input = "f(a, b) + g(c, d)";
        assert_eq!(input.extract_nth_balanced('(', 0), Some("a, b".to_string()));
        assert_eq!(input.extract_nth_balanced('(', 1), Some("c, d".to_string()));
        assert_eq!(input.extract_nth_balanced('(', 2), None);
        // nested brackets are balanced
        assert_eq!(
            "f(g(x), y) + h(z)".extract_nth_balanced('(', 0),
            Some("g(x), y".to_string())
        );
        assert_eq!(
            "f(g(x), y) + h(z)".extract_nth_balanced('(', 1),
            Some("z".to_string())
        );
    }

    #[test]
    fn test_extract_all_balanced() {
        let input = "f(a, b) + g(c, d)";
        assert_eq!(
            input.extract_all_balanced('('),
            vec!["a, b".to_string(), "c, d".to_string()]
        );
        // nesting is balanced: outer content includes inner brackets
        assert_eq!(
            "f(g(x), y) + h(z)".extract_all_balanced('('),
            vec!["g(x), y".to_string(), "z".to_string()]
        );
        // quoted closing delimiters are skipped
        let call = r#"f("a)", b) + g("c")"#;
        assert_eq!(
            call.extract_all_balanced('('),
            vec![r#""a)", b"#.to_string(), r#""c""#.to_string()]
        );
    }

    #[test]
    fn test_extract_all_from_parentheses() {
        let input = "f(a, b) + g(c, d)";
        assert_eq!(
            input.extract_all_from_parentheses(),
            vec![
                CapturedSegment::Outside("f"),
                CapturedSegment::Enclosure(Cow::Borrowed("a, b")),
                CapturedSegment::Outside(" + g"),
                CapturedSegment::Enclosure(Cow::Borrowed("c, d")),
            ]
        );
    }

    #[test]
    fn test_extract_all_from_brackets() {
        assert_eq!(
            "a[0] + b[1]".extract_all_from_brackets(),
            vec![
                CapturedSegment::Outside("a"),
                CapturedSegment::Enclosure(Cow::Borrowed("0")),
                CapturedSegment::Outside(" + b"),
                CapturedSegment::Enclosure(Cow::Borrowed("1")),
            ]
        );
    }

    #[test]
    fn test_extract_all_from_braces() {
        assert_eq!(
            "a{x} + b{y}".extract_all_from_braces(),
            vec![
                CapturedSegment::Outside("a"),
                CapturedSegment::Enclosure(Cow::Borrowed("x")),
                CapturedSegment::Outside(" + b"),
                CapturedSegment::Enclosure(Cow::Borrowed("y")),
            ]
        );
    }

    // --- multi-character delimiters and escape styles ---------------------

    #[test]
    fn test_extract_with_str_delimiters() {
        assert_eq!(
            "<!--a comment-->tail".extract_first("<!--", "-->"),
            Some("a comment".to_string())
        );
        assert_eq!("[[x]]".extract_first("[[", "]]"), Some("x".to_string()));
        assert_eq!("(?=y)".extract_first("(?=", ')'), Some("y".to_string()));
    }

    #[test]
    fn test_str_delimiter_folds_away_the_prefix() {
        let sample = "prefix!(contents_1)inbetween!(contents_2)suffix";
        let with_prefix = sample.extract_enclosures("!(", ')', EscapeStyle::None);
        let with_str_start = sample.extract_enclosures("!(", ')', EscapeStyle::None);
        assert_eq!(with_prefix, with_str_start);
        assert_eq!(
            with_str_start
                .iter()
                .filter_map(|s| s.enclosure())
                .collect::<Vec<_>>(),
            vec!["contents_1", "contents_2"]
        );
    }

    #[test]
    fn test_str_delimiter_escaping() {
        assert_eq!(
            r#"<!--a \--> b-->"#.extract_first_enclosure("<!--", "-->", EscapeStyle::Char('\\')),
            Some("a --> b".to_string())
        );
    }

    #[test]
    fn test_empty_delimiter_never_matches() {
        assert_eq!("anything".extract_first("", ")"), None);
        assert_eq!(
            "anything".extract_enclosures("", ")", EscapeStyle::None),
            vec![CapturedSegment::Outside("anything")]
        );
    }

    #[test]
    fn test_escape_style_doubled_is_explicit() {
        assert_eq!(
            r#""a ""quoted"" bit""#.extract_first_enclosure('"', '"', EscapeStyle::Doubled),
            Some(r#"a "quoted" bit"#.to_string())
        );
        assert_eq!(
            r#""a ""quoted"" bit""#.extract_first_enclosure('"', '"', EscapeStyle::Char('"')),
            Some(r#"a "quoted" bit"#.to_string())
        );
        assert_eq!(
            r#""a ""quoted"" bit""#.extract_first_enclosure('"', '"', EscapeStyle::None),
            Some("a ".to_string())
        );
    }

    #[test]
    fn test_enclose_extract_round_trip_across_styles() {
        let original = r#"has ) and " inside"#;
        for (style, opening) in [
            (EscapeStyle::Char('\\'), '('),
            (EscapeStyle::Doubled, '"'),
            (EscapeStyle::Char('\\'), '"'),
        ] {
            let closing = crate::match_closing_char(opening);
            let wrapped = original.enclose_escaped(opening, closing, style);
            assert_eq!(
                wrapped.extract_first_enclosure(opening, closing, style),
                Some(original.to_string()),
                "round trip failed for {opening:?} with {style:?}"
            );
        }
    }

    #[test]
    fn test_extract_enclosed_doubled_convenience() {
        assert_eq!(
            r#""a ""b"" c""#.extract_enclosed_doubled('"'),
            Some(r#"a "b" c"#.to_string())
        );
    }

    // --- extract_first_enclosure ------------------------------------------

    #[test]
    fn test_extract_first_enclosure() {
        assert_eq!(
            "a(one)b(two)c".extract_first_enclosure('(', ')', EscapeStyle::None),
            Some("one".to_string())
        );
        assert_eq!(
            "skip(this)!(wanted)".extract_first_enclosure("!(", ')', EscapeStyle::None),
            Some("wanted".to_string())
        );
        assert_eq!(
            r#"(a\) b)"#.extract_first_enclosure('(', ')', EscapeStyle::Char('\\')),
            Some("a) b".to_string())
        );
        assert_eq!(
            r#""a ""q"" b""#.extract_first_enclosure('"', '"', EscapeStyle::Doubled),
            Some(r#"a "q" b"#.to_string())
        );
    }

    #[test]
    fn test_extract_first_enclosure_none_cases() {
        assert_eq!(
            "no brackets".extract_first_enclosure('(', ')', EscapeStyle::None),
            None
        );
        assert_eq!(
            "(unterminated".extract_first_enclosure('(', ')', EscapeStyle::None),
            None
        );
    }

    #[test]
    fn test_first_enclosure_agrees_with_the_base_method() {
        for sample in ["x(one)y(two)z", "(only)", "none here", "(unterminated"] {
            let via_base = sample
                .extract_enclosures('(', ')', EscapeStyle::None)
                .into_iter()
                .find_map(|segment| segment.enclosure().map(str::to_string));
            assert_eq!(
                sample.extract_first_enclosure('(', ')', EscapeStyle::None),
                via_base,
                "disagreement on {sample:?}"
            );
        }
    }

    // --- quoted regions and nesting ---------------------------------------

    #[test]
    fn test_extract_balanced_skips_quoted_closing_delimiter() {
        let call = r#"add_title("Latest Stats (2024-2025)", "en-GB")"#;
        assert_eq!(
            call.extract_balanced('('),
            Some(r#""Latest Stats (2024-2025)", "en-GB""#.to_string())
        );
    }

    #[test]
    fn test_quoted_region_beats_nesting() {
        let call = r#"add_title("Smiley :)", "en")"#;
        assert_eq!(
            call.extract_balanced('('),
            Some(r#""Smiley :)", "en""#.to_string())
        );
        assert_eq!(
            r#"f("a (b", "c")"#.extract_balanced('('),
            Some(r#""a (b", "c""#.to_string())
        );
    }

    #[test]
    fn test_balanced_nesting() {
        assert_eq!(
            "f(g(x), y)".extract_balanced('('),
            Some("g(x), y".to_string())
        );
        assert_eq!(
            "outer(a(b(c)), d)".extract_balanced('('),
            Some("a(b(c)), d".to_string())
        );
        assert_eq!(
            "f(g(x), y)".extract_first_enclosure('(', ')', EscapeStyle::None),
            Some("g(x".to_string())
        );
    }

    #[test]
    fn test_escaped_quote_inside_a_quoted_region() {
        let call = r#"f("say \"hi\" now", "x")"#;
        assert_eq!(
            call.extract_balanced('('),
            Some(r#""say \"hi\" now", "x""#.to_string())
        );
    }

    #[test]
    fn test_single_and_double_quotes_both_recognised() {
        assert_eq!(
            r#"f('a )b', "c )d")"#.extract_balanced('('),
            Some(r#"'a )b', "c )d""#.to_string())
        );
        assert_eq!(
            r#"f("it's fine)")"#.extract_balanced('('),
            Some(r#""it's fine)""#.to_string())
        );
    }

    #[test]
    fn test_scan_options_are_composable() {
        let options = ScanOptions::default().with_quotes(&['"']);
        assert_eq!(
            r#"("a )b")tail"#.extract_first_enclosure_with('(', ')', options),
            Some(r#""a )b""#.to_string())
        );
        assert_eq!(
            r#"("a )b")tail"#.extract_first_enclosure('(', ')', EscapeStyle::None),
            Some(r#""a "#.to_string())
        );
    }

    #[test]
    fn test_balanced_ignored_for_identical_delimiters() {
        let options = ScanOptions::default().with_nesting(Nesting::Delimiters);
        assert_eq!(
            r#""one" and "two""#.extract_first_enclosure_with('"', '"', options),
            Some("one".to_string())
        );
    }

    #[test]
    fn test_unterminated_quote_leaves_the_enclosure_unclosed() {
        assert_eq!(r#"f("never closed)"#.extract_balanced('('), None);
    }

    #[test]
    fn test_two_stage_call_parse() {
        let call = r#"add_title("Latest Stats (2024-2025)", "en-GB")"#;
        let args = call.extract_balanced('(').expect("balanced enclosure");
        assert_eq!(args, r#""Latest Stats (2024-2025)", "en-GB""#);
        assert_eq!(
            args.extract_all_enclosed('"'),
            vec!["Latest Stats (2024-2025)".to_string(), "en-GB".to_string()]
        );
    }

    #[test]
    fn test_named_delimiter_conveniences() {
        assert_eq!(
            "func_name(arg)".extract_from_parentheses(),
            Some("arg".to_string())
        );
        assert_eq!("a[0]".extract_from_brackets(), Some("0".to_string()));
        assert_eq!("x{y}z".extract_from_braces(), Some("y".to_string()));
        assert_eq!(
            r#"say "hi" now"#.extract_from_double_quotes(),
            Some("hi".to_string())
        );
        // The apostrophe opens the group, so this captures "s " rather than
        // "quoted". That is the defined rule applied consistently: an
        // apostrophe and a single quote are one code point, and distinguishing
        // them needs sentence-level context, which is not this crate's job.
        assert_eq!(
            "it's 'quoted' here".extract_from_single_quotes(),
            Some("s ".to_string())
        );
        assert_eq!("plain text".extract_from_parentheses(), None);
    }

    #[test]
    fn test_opening_delimiter_carries_its_own_prefix() {
        // a regex lookbehind: the "?<=" belongs to the opening delimiter
        assert_eq!(
            "(?<=foo)bar".extract_first_enclosure("(?<=", ')', EscapeStyle::None),
            Some("foo".to_string())
        );
        // a marker before the bracket is equally just part of the delimiter
        assert_eq!(
            "x!(y)".extract_first_enclosure("!(", ')', EscapeStyle::None),
            Some("y".to_string())
        );
        // balanced tracks the opening delimiter as given, so a bare "(" inside
        // a "(?<=" group is not counted and the first ")" closes
        assert_eq!(
            "(?<=(a|b))rest".extract_first_enclosure_with(
                "(?<=",
                ')',
                ScanOptions::default().with_nesting(Nesting::Delimiters)
            ),
            Some("(a|b".to_string())
        );
        // balancing on the bare bracket captures the group, at the cost of not
        // requiring the lookbehind marker
        assert_eq!(
            "(?<=(a|b))rest".extract_first_enclosure_with(
                '(',
                ')',
                ScanOptions::default().with_nesting(Nesting::Delimiters)
            ),
            Some("?<=(a|b)".to_string())
        );
    }

    // --- parsing a key spec, the intended use case -----------------------

    #[test]
    fn test_parse_dsl_key_spec() {
        // e.g. --keys "valid_codes|int[]:(,)"
        // split_once stands in for to_head_tail from the to_segments crate,
        // which is not a dependency here.
        let spec = "valid_codes|int[]:(,)";
        let (field, rest) = spec.split_once('|').expect("field name");
        assert_eq!(field, "valid_codes");
        let (cast, tail) = rest.split_once(':').expect("cast and splitter");
        assert_eq!(cast, "int[]");
        assert_eq!(tail.extract_enclosed('('), Some(",".to_string()));
    }

    #[test]
    fn test_parse_dsl_splitter_variants() {
        assert_eq!("(, )".extract_enclosed('('), Some(", ".to_string()));
        assert_eq!("( | )".extract_enclosed('('), Some(" | ".to_string()));
        assert_eq!(r#"(\))"#.extract_enclosed('('), Some(")".to_string()));
        assert_eq!("()".extract_enclosed('('), Some(String::new()));
        assert_eq!("int[]".extract_enclosed('('), None);
    }

    #[test]
    fn test_parse_dsl_spec_without_splitter() {
        let spec = "name|str";
        let (field, rest) = spec.split_once('|').expect("field name");
        assert_eq!(field, "name");
        assert_eq!(rest.split_once(':'), None);
        assert_eq!(rest.extract_enclosed('('), None);
    }
}

#[cfg(test)]
mod nesting_tests {
    use super::*;

    #[test]
    fn test_nesting_pair_differs_from_the_bounding_delimiters() {
        // the lookbehind case: opening delimiter carries a prefix, but the
        // groups to balance are plain brackets
        assert_eq!(
            "(?<=(a|b))rest".extract_first_enclosure_with(
                "(?<=",
                ')',
                ScanOptions::default().with_nesting(Nesting::Pair('(', ')'))
            ),
            Some("(a|b)".to_string())
        );
        // Nesting::Delimiters counts the opening delimiter as given, so the
        // inner bracket is not tracked and the first ) closes
        assert_eq!(
            "(?<=(a|b))rest".extract_first_enclosure_with(
                "(?<=",
                ')',
                ScanOptions::default().with_nesting(Nesting::Delimiters)
            ),
            Some("(a|b".to_string())
        );
    }

    #[test]
    fn test_nesting_default_is_none() {
        assert_eq!(ScanOptions::default().nesting, Nesting::None);
        assert_eq!(
            "f(g(x), y)".extract_first_enclosure('(', ')', EscapeStyle::None),
            Some("g(x".to_string())
        );
    }
}


/// A bounding token: either a single `char` or a `&str` of any length.
///
/// Implemented for `char`, `&str` and `String`, so the same call can mix them:
/// `text.enclose_in("(?=", ')', EscapeStyle::None)`.
///
/// All positions are byte indices into the haystack, which keeps multi-byte
/// characters correct by construction. An empty string delimiter never matches,
/// since a zero-width token would match at every position.
pub trait Delimiter {
    /// Byte length of this delimiter if it occurs at `at`, otherwise `None`.
    fn match_len(&self, hay: &str, at: usize) -> Option<usize>;

    /// Byte index of the next occurrence at or after `from`.
    fn find_from(&self, hay: &str, from: usize) -> Option<usize>;

    /// Byte length of the delimiter itself.
    fn byte_len(&self) -> usize;

    /// Append the delimiter to a buffer.
    fn push_to(&self, out: &mut String);

    /// Insert the delimiter at a byte index.
    fn insert_into(&self, out: &mut String, index: usize);

    /// True when the delimiter can never match.
    fn is_empty(&self) -> bool {
        self.byte_len() == 0
    }
}

impl Delimiter for char {
    fn match_len(&self, hay: &str, at: usize) -> Option<usize> {
        hay[at..].starts_with(*self).then_some(self.len_utf8())
    }

    fn find_from(&self, hay: &str, from: usize) -> Option<usize> {
        hay[from..].find(*self).map(|index| from + index)
    }

    fn byte_len(&self) -> usize {
        self.len_utf8()
    }

    fn push_to(&self, out: &mut String) {
        out.push(*self);
    }

    fn insert_into(&self, out: &mut String, index: usize) {
        out.insert(index, *self);
    }
}

impl Delimiter for &str {
    fn match_len(&self, hay: &str, at: usize) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        hay[at..].starts_with(*self).then_some(self.len())
    }

    fn find_from(&self, hay: &str, from: usize) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        hay[from..].find(*self).map(|index| from + index)
    }

    fn byte_len(&self) -> usize {
        self.len()
    }

    fn push_to(&self, out: &mut String) {
        out.push_str(self);
    }

    fn insert_into(&self, out: &mut String, index: usize) {
        out.insert_str(index, self);
    }
}

impl Delimiter for String {
    fn match_len(&self, hay: &str, at: usize) -> Option<usize> {
        self.as_str().match_len(hay, at)
    }

    fn find_from(&self, hay: &str, from: usize) -> Option<usize> {
        self.as_str().find_from(hay, from)
    }

    fn byte_len(&self) -> usize {
        self.len()
    }

    fn push_to(&self, out: &mut String) {
        out.push_str(self);
    }

    fn insert_into(&self, out: &mut String, index: usize) {
        out.insert_str(index, self);
    }
}

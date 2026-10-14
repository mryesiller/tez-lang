use tez_diag::{Pos, Span};

/// A half-open byte range `[start, end)` into a source string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteSpan {
    /// First byte inside the range.
    pub start: usize,
    /// First byte after the range.
    pub end: usize,
}

impl ByteSpan {
    /// A range `[start, end)`.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// The smallest range covering both.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// An empty range at the end of this one (an insertion point).
    #[must_use]
    pub const fn end_point(self) -> Self {
        Self {
            start: self.end,
            end: self.end,
        }
    }

    /// The text this range covers.
    #[must_use]
    pub fn text(self, src: &str) -> &str {
        src.get(self.start..self.end).unwrap_or("")
    }
}

/// Maps byte offsets to line/column positions for one source text.
#[derive(Debug, Clone)]
pub struct LineIndex<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    /// Indexes `text`. Lines end at `\n`; a preceding `\r` counts as part of the line.
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { text, line_starts }
    }

    /// Converts a byte offset to a 1-based line and a 1-based column counted in chars.
    /// Offsets past the end clamp to the end; offsets inside a char round down to its start.
    #[must_use]
    pub fn pos(&self, offset: usize) -> Pos {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let col = self.text[self.line_starts[line]..offset].chars().count();
        Pos {
            line: to_u32(line) + 1,
            col: to_u32(col) + 1,
        }
    }

    /// Converts a byte range to a [`Span`].
    #[must_use]
    pub fn span(&self, range: ByteSpan) -> Span {
        Span {
            start: self.pos(range.start),
            end: self.pos(range.end),
        }
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(line: u32, col: u32) -> Pos {
        Pos { line, col }
    }

    #[test]
    fn first_position_is_1_1() {
        assert_eq!(LineIndex::new("abc").pos(0), p(1, 1));
    }

    #[test]
    fn moves_to_next_line_after_newline() {
        let idx = LineIndex::new("ab\ncd");
        assert_eq!(idx.pos(2), p(1, 3)); // the '\n' itself
        assert_eq!(idx.pos(3), p(2, 1));
        assert_eq!(idx.pos(4), p(2, 2));
    }

    #[test]
    fn columns_count_chars_not_bytes() {
        let idx = LineIndex::new("é=1"); // 'é' is 2 bytes
        assert_eq!(idx.pos(2), p(1, 2));
        assert_eq!(idx.pos(1), p(1, 1)); // inside 'é' rounds down
    }

    #[test]
    fn offset_past_end_clamps() {
        let idx = LineIndex::new("a\n");
        assert_eq!(idx.pos(999), p(2, 1));
    }

    #[test]
    fn empty_text() {
        assert_eq!(LineIndex::new("").pos(0), p(1, 1));
    }

    #[test]
    fn span_converts_both_ends() {
        let idx = LineIndex::new("ab\ncd");
        let s = idx.span(ByteSpan { start: 1, end: 4 });
        assert_eq!(s.start, p(1, 2));
        assert_eq!(s.end, p(2, 2));
    }
}

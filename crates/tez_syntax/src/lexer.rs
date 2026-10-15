use tez_diag::Code;

use crate::problem::Problem;
use crate::source::ByteSpan;

/// Reserved words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kw {
    /// `fn`
    Fn,
    /// `type`
    Type,
    /// `const`
    Const,
    /// `test`
    Test,
    /// `let`
    Let,
    /// `match`
    Match,
    /// `if`
    If,
    /// `else`
    Else,
    /// `where`
    Where,
    /// `with`
    With,
    /// `requires`
    Requires,
    /// `ensures`
    Ensures,
    /// `decreases`
    Decreases,
    /// `assert`
    Assert,
    /// `extern`
    Extern,
    /// `use`
    Use,
    /// `true`
    True,
    /// `false`
    False,
}

impl Kw {
    /// Source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Kw::Fn => "fn",
            Kw::Type => "type",
            Kw::Const => "const",
            Kw::Test => "test",
            Kw::Let => "let",
            Kw::Match => "match",
            Kw::If => "if",
            Kw::Else => "else",
            Kw::Where => "where",
            Kw::With => "with",
            Kw::Requires => "requires",
            Kw::Ensures => "ensures",
            Kw::Decreases => "decreases",
            Kw::Assert => "assert",
            Kw::Extern => "extern",
            Kw::Use => "use",
            Kw::True => "true",
            Kw::False => "false",
        }
    }

    fn from_str(s: &str) -> Option<Kw> {
        Some(match s {
            "fn" => Kw::Fn,
            "type" => Kw::Type,
            "const" => Kw::Const,
            "test" => Kw::Test,
            "let" => Kw::Let,
            "match" => Kw::Match,
            "if" => Kw::If,
            "else" => Kw::Else,
            "where" => Kw::Where,
            "with" => Kw::With,
            "requires" => Kw::Requires,
            "ensures" => Kw::Ensures,
            "decreases" => Kw::Decreases,
            "assert" => Kw::Assert,
            "extern" => Kw::Extern,
            "use" => Kw::Use,
            "true" => Kw::True,
            "false" => Kw::False,
            _ => return None,
        })
    }
}

/// Token kinds. Literal and identifier text is carried in the payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    /// Integer literal, digits only (underscores removed).
    Int(String),
    /// Float literal.
    Float(String),
    /// String literal, escapes decoded.
    Str(String),
    /// Identifier starting with a lowercase letter.
    Lower(String),
    /// Identifier starting with an uppercase letter.
    Upper(String),
    /// A lone `_`.
    Underscore,
    /// Reserved word.
    Kw(Kw),
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `;` (always an error in Tez; lexed so the parser can explain)
    Semi,
    /// `.`
    Dot,
    /// `..`
    DotDot,
    /// `=`
    Eq,
    /// `=>`
    FatArrow,
    /// `->`
    Arrow,
    /// `|`
    Bar,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `++`
    PlusPlus,
    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    AndAnd,
    /// `||`
    OrOr,
    /// `!`
    Bang,
    /// One or more line breaks.
    Newline,
    /// End of input.
    Eof,
}

impl Tok {
    /// Human-readable description for messages.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Tok::Int(s) | Tok::Float(s) => format!("`{s}`"),
            Tok::Str(_) => "a string".into(),
            Tok::Lower(s) | Tok::Upper(s) => format!("`{s}`"),
            Tok::Underscore => "`_`".into(),
            Tok::Kw(k) => format!("`{}`", k.as_str()),
            Tok::LParen => "`(`".into(),
            Tok::RParen => "`)`".into(),
            Tok::LBracket => "`[`".into(),
            Tok::RBracket => "`]`".into(),
            Tok::LBrace => "`{`".into(),
            Tok::RBrace => "`}`".into(),
            Tok::Comma => "`,`".into(),
            Tok::Colon => "`:`".into(),
            Tok::Semi => "`;`".into(),
            Tok::Dot => "`.`".into(),
            Tok::DotDot => "`..`".into(),
            Tok::Eq => "`=`".into(),
            Tok::FatArrow => "`=>`".into(),
            Tok::Arrow => "`->`".into(),
            Tok::Bar => "`|`".into(),
            Tok::Plus => "`+`".into(),
            Tok::Minus => "`-`".into(),
            Tok::Star => "`*`".into(),
            Tok::Slash => "`/`".into(),
            Tok::Percent => "`%`".into(),
            Tok::PlusPlus => "`++`".into(),
            Tok::EqEq => "`==`".into(),
            Tok::NotEq => "`!=`".into(),
            Tok::Lt => "`<`".into(),
            Tok::Le => "`<=`".into(),
            Tok::Gt => "`>`".into(),
            Tok::Ge => "`>=`".into(),
            Tok::AndAnd => "`&&`".into(),
            Tok::OrOr => "`||`".into(),
            Tok::Bang => "`!`".into(),
            Tok::Newline => "a line break".into(),
            Tok::Eof => "end of input".into(),
        }
    }
}

/// A token with its source range.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// Kind and payload.
    pub tok: Tok,
    /// Source range.
    pub span: ByteSpan,
}

/// A `//` comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// Range including the `//`.
    pub span: ByteSpan,
    /// True when only whitespace precedes it on its line.
    pub own_line: bool,
}

/// Result of lexing.
#[derive(Debug, Clone)]
pub struct LexOutput {
    /// Tokens, ending with [`Tok::Eof`].
    pub tokens: Vec<Token>,
    /// Comments in source order.
    pub comments: Vec<Comment>,
    /// Lexical problems.
    pub problems: Vec<Problem>,
}

/// Lexes `src`. Never fails: problems are collected and lexing continues.
#[must_use]
pub fn lex(src: &str) -> LexOutput {
    let mut lx = Lexer {
        src,
        pos: 0,
        tokens: Vec::new(),
        comments: Vec::new(),
        problems: Vec::new(),
    };
    lx.run();
    lx.tokens.push(Token {
        tok: Tok::Eof,
        span: ByteSpan::new(src.len(), src.len()),
    });
    LexOutput {
        tokens: lx.tokens,
        comments: lx.comments,
        problems: lx.problems,
    }
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    problems: Vec<Problem>,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn push(&mut self, tok: Tok, start: usize) {
        self.tokens.push(Token {
            tok,
            span: ByteSpan::new(start, self.pos),
        });
    }

    fn problem(&mut self, code: Code, msg: &str, span: ByteSpan) {
        self.problems.push(Problem::new(code, msg, span));
    }

    fn after_dot(&self) -> bool {
        matches!(self.tokens.last(), Some(Token { tok: Tok::Dot, .. }))
    }

    fn run(&mut self) {
        while let Some(c) = self.peek() {
            let start = self.pos;
            match c {
                ' ' | '\r' => self.pos += 1,
                '\t' => {
                    self.pos += 1;
                    let span = ByteSpan::new(start, self.pos);
                    self.problems.push(
                        Problem::new(Code::LexUnexpectedChar, "tab character", span).fix(
                            "use spaces",
                            span,
                            "  ",
                        ),
                    );
                }
                '\n' => {
                    self.pos += 1;
                    if !matches!(
                        self.tokens.last(),
                        None | Some(Token {
                            tok: Tok::Newline,
                            ..
                        })
                    ) {
                        self.push(Tok::Newline, start);
                    }
                }
                '/' if self.peek_at(1) == Some('/') => self.comment(start),
                '"' => self.string(start),
                c if c.is_ascii_digit() => self.number(start),
                c if c.is_ascii_alphabetic() || c == '_' => self.ident(start),
                _ => self.punct(c, start),
            }
        }
    }

    fn comment(&mut self, start: usize) {
        let line_start = self.src[..start].rfind('\n').map_or(0, |i| i + 1);
        let own_line = self.src[line_start..start].trim().is_empty();
        let end = self.src[start..]
            .find('\n')
            .map_or(self.src.len(), |i| start + i);
        self.pos = end;
        self.comments.push(Comment {
            span: ByteSpan::new(start, end),
            own_line,
        });
    }

    fn ident(&mut self, start: usize) {
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            self.pos += 1;
        }
        let text = &self.src[start..self.pos];
        let tok = if text == "_" {
            Tok::Underscore
        } else if let Some(kw) = Kw::from_str(text) {
            Tok::Kw(kw)
        } else if text.starts_with(|c: char| c.is_ascii_uppercase()) {
            Tok::Upper(text.to_string())
        } else {
            Tok::Lower(text.to_string())
        };
        self.push(tok, start);
    }

    fn digits(&mut self) -> bool {
        let begin = self.pos;
        let mut ok = true;
        let mut prev_us = true; // a leading underscore is impossible here, so treat start as "after underscore" to catch `_` first
        let mut first = true;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                prev_us = false;
            } else if c == '_' && !first {
                if prev_us {
                    ok = false;
                }
                prev_us = true;
            } else {
                break;
            }
            first = false;
            self.pos += 1;
        }
        if prev_us && self.pos > begin {
            ok = false; // trailing underscore
        }
        ok
    }

    fn number(&mut self, start: usize) {
        let mut ok = self.digits();
        let mut is_float = false;
        if !self.after_dot()
            && self.peek() == Some('.')
            && matches!(self.peek_at(1), Some(c) if c.is_ascii_digit())
        {
            self.pos += 1;
            ok &= self.digits();
            is_float = true;
        }
        // `12abc`, `0x1F`, `1.5e3`: a letter glued to a number
        if matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
            while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
                self.pos += 1;
            }
            ok = false;
        }
        let span = ByteSpan::new(start, self.pos);
        if !ok {
            self.problem(Code::LexBadNumber, "malformed number", span);
        }
        let text: String = self.src[start..self.pos]
            .chars()
            .filter(|&c| c != '_')
            .collect();
        self.push(
            if is_float {
                Tok::Float(text)
            } else {
                Tok::Int(text)
            },
            start,
        );
    }

    fn string(&mut self, start: usize) {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    let span = ByteSpan::new(start, self.pos);
                    self.problems.push(
                        Problem::new(Code::LexUnterminatedString, "unterminated string", span).fix(
                            "close the string",
                            span.end_point(),
                            "\"",
                        ),
                    );
                    break;
                }
                Some('"') => {
                    self.pos += 1;
                    break;
                }
                Some('\\') => self.escape(&mut out),
                Some(c) => {
                    out.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
        self.push(Tok::Str(out), start);
    }

    fn escape(&mut self, out: &mut String) {
        let esc_start = self.pos;
        self.pos += 1; // backslash
        let Some(c) = self.peek() else { return };
        self.pos += c.len_utf8();
        match c {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '0' => out.push('\0'),
            'u' => {
                let ok = self.peek() == Some('{');
                let mut code = String::new();
                if ok {
                    self.pos += 1;
                    while let Some(h) = self.peek() {
                        if h == '}' {
                            self.pos += 1;
                            break;
                        }
                        if !h.is_ascii_hexdigit() || code.len() > 6 {
                            break;
                        }
                        code.push(h);
                        self.pos += 1;
                    }
                }
                match u32::from_str_radix(&code, 16).ok().and_then(char::from_u32) {
                    Some(ch) if ok && !code.is_empty() => out.push(ch),
                    _ => self.problem(
                        Code::LexBadEscape,
                        "bad unicode escape",
                        ByteSpan::new(esc_start, self.pos),
                    ),
                }
            }
            _ => self.problem(
                Code::LexBadEscape,
                "unknown escape",
                ByteSpan::new(esc_start, self.pos),
            ),
        }
    }

    fn punct(&mut self, c: char, start: usize) {
        let next = self.peek_at(1);
        let (tok, len) = match (c, next) {
            ('(', _) => (Tok::LParen, 1),
            (')', _) => (Tok::RParen, 1),
            ('[', _) => (Tok::LBracket, 1),
            (']', _) => (Tok::RBracket, 1),
            ('{', _) => (Tok::LBrace, 1),
            ('}', _) => (Tok::RBrace, 1),
            (',', _) => (Tok::Comma, 1),
            (':', _) => (Tok::Colon, 1),
            (';', _) => (Tok::Semi, 1),
            ('.', Some('.')) => (Tok::DotDot, 2),
            ('.', _) => (Tok::Dot, 1),
            ('=', Some('=')) => (Tok::EqEq, 2),
            ('=', Some('>')) => (Tok::FatArrow, 2),
            ('=', _) => (Tok::Eq, 1),
            ('-', Some('>')) => (Tok::Arrow, 2),
            ('-', _) => (Tok::Minus, 1),
            ('+', Some('+')) => (Tok::PlusPlus, 2),
            ('+', _) => (Tok::Plus, 1),
            ('*', _) => (Tok::Star, 1),
            ('/', _) => (Tok::Slash, 1),
            ('%', _) => (Tok::Percent, 1),
            ('!', Some('=')) => (Tok::NotEq, 2),
            ('!', _) => (Tok::Bang, 1),
            ('<', Some('=')) => (Tok::Le, 2),
            ('<', _) => (Tok::Lt, 1),
            ('>', Some('=')) => (Tok::Ge, 2),
            ('>', _) => (Tok::Gt, 1),
            ('&', Some('&')) => (Tok::AndAnd, 2),
            ('|', Some('|')) => (Tok::OrOr, 2),
            ('|', _) => (Tok::Bar, 1),
            _ => {
                self.pos += c.len_utf8();
                let span = ByteSpan::new(start, self.pos);
                self.problems.push(
                    Problem::new(
                        Code::LexUnexpectedChar,
                        format!("unexpected character `{c}`"),
                        span,
                    )
                    .fix("delete it", span, ""),
                );
                return;
            }
        };
        self.pos += len;
        self.push(tok, start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tok> {
        lex(src).tokens.into_iter().map(|t| t.tok).collect()
    }

    fn codes(src: &str) -> Vec<Code> {
        lex(src).problems.iter().map(|p| p.code).collect()
    }

    #[test]
    fn basic_tokens() {
        assert_eq!(
            kinds("fn f(x: Int) -> Int = x + 1"),
            vec![
                Tok::Kw(Kw::Fn),
                Tok::Lower("f".into()),
                Tok::LParen,
                Tok::Lower("x".into()),
                Tok::Colon,
                Tok::Upper("Int".into()),
                Tok::RParen,
                Tok::Arrow,
                Tok::Upper("Int".into()),
                Tok::Eq,
                Tok::Lower("x".into()),
                Tok::Plus,
                Tok::Int("1".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn operators_are_greedy() {
        assert_eq!(
            kinds("== != <= >= && || ++ => -> .."),
            vec![
                Tok::EqEq,
                Tok::NotEq,
                Tok::Le,
                Tok::Ge,
                Tok::AndAnd,
                Tok::OrOr,
                Tok::PlusPlus,
                Tok::FatArrow,
                Tok::Arrow,
                Tok::DotDot,
                Tok::Eof
            ]
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(
            kinds("1_000 2.5"),
            vec![Tok::Int("1000".into()), Tok::Float("2.5".into()), Tok::Eof]
        );
        assert!(codes("1__0").contains(&Code::LexBadNumber));
        assert!(codes("1_").contains(&Code::LexBadNumber));
        assert!(codes("0x1F").contains(&Code::LexBadNumber));
        assert!(codes("12abc").contains(&Code::LexBadNumber));
    }

    #[test]
    fn method_on_int_literal_and_tuple_index() {
        assert_eq!(
            kinds("1.abs()"),
            vec![
                Tok::Int("1".into()),
                Tok::Dot,
                Tok::Lower("abs".into()),
                Tok::LParen,
                Tok::RParen,
                Tok::Eof
            ]
        );
        assert_eq!(
            kinds("t.0.1"),
            vec![
                Tok::Lower("t".into()),
                Tok::Dot,
                Tok::Int("0".into()),
                Tok::Dot,
                Tok::Int("1".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn strings_and_escapes() {
        assert_eq!(
            kinds(r#""a\n\"b\u{41}""#),
            vec![Tok::Str("a\n\"bA".into()), Tok::Eof]
        );
        assert!(codes(r#""a\qb""#).contains(&Code::LexBadEscape));
        assert!(codes("\"open").contains(&Code::LexUnterminatedString));
    }

    #[test]
    fn newlines_collapse_and_comments_are_trivia() {
        let out = lex("a // hi\n\n\n// own\nb");
        let ks: Vec<_> = out.tokens.iter().map(|t| t.tok.clone()).collect();
        assert_eq!(
            ks,
            vec![
                Tok::Lower("a".into()),
                Tok::Newline,
                Tok::Lower("b".into()),
                Tok::Eof
            ]
        );
        assert_eq!(out.comments.len(), 2);
        assert!(!out.comments[0].own_line);
        assert!(out.comments[1].own_line);
    }

    #[test]
    fn stray_characters_and_tabs() {
        assert!(codes("a @ b").contains(&Code::LexUnexpectedChar));
        assert!(codes("a\tb").contains(&Code::LexUnexpectedChar));
        assert!(codes("é").contains(&Code::LexUnexpectedChar));
        assert!(codes("a & b").contains(&Code::LexUnexpectedChar));
    }

    #[test]
    fn keywords_underscore_and_cases() {
        assert_eq!(
            kinds("_ _x Foo let"),
            vec![
                Tok::Underscore,
                Tok::Lower("_x".into()),
                Tok::Upper("Foo".into()),
                Tok::Kw(Kw::Let),
                Tok::Eof
            ]
        );
    }
}

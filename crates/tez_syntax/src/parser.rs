//! Hand-written recursive-descent parser with error recovery.
//!
//! Recovery strategy: an error aborts the current statement, match arm or item; the parser
//! then skips to the next line (statement/arm) or to the next line-initial item keyword
//! (item) and continues, so one run reports every independent syntax problem.

use tez_diag::Code;

use crate::ast::{
    Arm, BinOp, ConstDecl, Expr, ExprKind, FieldDecl, FieldInit, File, FnDecl, Ident, Item,
    LambdaParam, Lit, Param, Pat, PatKind, Stmt, TestDecl, TypeBody, TypeDecl, TypeExpr, TypeKind,
    UnOp, Variant,
};
use crate::lexer::{Comment, Kw, Tok, Token, lex};
use crate::names::{is_lower_camel, is_upper_camel, to_lower_camel, to_upper_camel};
use crate::problem::Problem;
use crate::source::ByteSpan;

/// Parser configuration.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseOptions {
    /// Prelude mode: allows `extern fn` and qualified `fn Module.name`.
    pub prelude: bool,
}

/// Result of parsing one file.
#[derive(Debug, Clone)]
pub struct ParseOutput {
    /// The syntax tree (possibly partial when problems were found).
    pub file: File,
    /// Lexical and syntax problems.
    pub problems: Vec<Problem>,
    /// Comments, for the formatter.
    pub comments: Vec<Comment>,
}

/// Parses `src`. Never panics; problems are returned alongside a best-effort tree.
#[must_use]
pub fn parse(src: &str, options: ParseOptions) -> ParseOutput {
    let lexed = lex(src);
    let mut p = Parser {
        src,
        toks: lexed.tokens,
        pos: 0,
        ignore_nl: false,
        no_struct: false,
        problems: lexed.problems,
        prelude: options.prelude,
        last_abort: usize::MAX,
    };
    let file = p.file();
    p.problems.sort_by_key(|pr| pr.span.start);
    ParseOutput {
        file,
        problems: p.problems,
        comments: lexed.comments,
    }
}

/// Marker for "an error was reported; unwind to a recovery point".
struct Abort;
type PResult<T> = Result<T, Abort>;

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Token>,
    pos: usize,
    /// Inside `( )` / `[ ]`: line breaks are insignificant.
    ignore_nl: bool,
    /// In `if`/`match` heads: `Name {` is not a record literal.
    no_struct: bool,
    problems: Vec<Problem>,
    prelude: bool,
    last_abort: usize,
}

fn is_cmp(t: &Tok) -> Option<BinOp> {
    Some(match t {
        Tok::EqEq => BinOp::Eq,
        Tok::NotEq => BinOp::Ne,
        Tok::Lt => BinOp::Lt,
        Tok::Le => BinOp::Le,
        Tok::Gt => BinOp::Gt,
        Tok::Ge => BinOp::Ge,
        _ => return None,
    })
}

impl Parser<'_> {
    // ---------------------------------------------------------------- token access

    fn sync(&mut self) {
        if self.ignore_nl {
            while matches!(self.toks[self.pos].tok, Tok::Newline) {
                self.pos += 1;
            }
        }
    }

    fn tok(&mut self) -> &Tok {
        self.sync();
        &self.toks[self.pos].tok
    }

    fn span(&mut self) -> ByteSpan {
        self.sync();
        self.toks[self.pos].span
    }

    fn prev_end(&self) -> usize {
        if self.pos == 0 {
            0
        } else {
            self.toks[self.pos - 1].span.end
        }
    }

    fn bump(&mut self) -> Token {
        self.sync();
        let t = self.toks[self.pos].clone();
        if !matches!(t.tok, Tok::Eof) {
            self.pos += 1;
        }
        t
    }

    fn at(&mut self, t: &Tok) -> bool {
        self.tok() == t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.at(t) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn at_kw(&mut self, k: Kw) -> bool {
        matches!(self.tok(), Tok::Kw(x) if *x == k)
    }

    /// Token `n` steps ahead of the current one (line breaks skipped only when ignored).
    fn peek_n(&mut self, n: usize) -> &Tok {
        self.sync();
        let mut i = self.pos;
        for _ in 0..n {
            if i + 1 < self.toks.len() {
                i += 1;
            }
            if self.ignore_nl {
                while i + 1 < self.toks.len() && matches!(self.toks[i].tok, Tok::Newline) {
                    i += 1;
                }
            }
        }
        &self.toks[i].tok
    }

    fn skip_newlines(&mut self) {
        while matches!(self.toks[self.pos].tok, Tok::Newline) {
            self.pos += 1;
        }
    }

    /// Index of the first non-newline token at or after `from`.
    fn next_sig(&self, from: usize) -> usize {
        let mut i = from;
        while i + 1 < self.toks.len() && matches!(self.toks[i].tok, Tok::Newline) {
            i += 1;
        }
        i
    }

    // ---------------------------------------------------------------- errors

    fn push(&mut self, p: Problem) {
        self.problems.push(p);
    }

    fn abort(&mut self, p: Problem) -> Abort {
        if self.last_abort != p.span.start {
            self.last_abort = p.span.start;
            self.problems.push(p);
        }
        Abort
    }

    fn unexpected(&mut self, expected: &str) -> Abort {
        self.sync();
        let t = self.toks[self.pos].clone();
        let p = if matches!(t.tok, Tok::Eof) {
            Problem::new(
                Code::ParseUnexpectedEof,
                format!("unexpected end of input, expected {expected}"),
                t.span,
            )
        } else {
            Problem::new(
                Code::ParseUnexpectedToken,
                format!("expected {expected}, found {}", t.tok.describe()),
                t.span,
            )
        };
        self.abort(p.expected(expected))
    }

    fn expect(&mut self, t: &Tok) -> PResult<ByteSpan> {
        if self.at(t) {
            Ok(self.bump().span)
        } else {
            Err(self.unexpected(&t.describe()))
        }
    }

    /// Expects a closing delimiter; at end of input reports the opener as unclosed.
    fn expect_close(&mut self, close: &Tok, open: ByteSpan) -> PResult<ByteSpan> {
        if self.at(close) {
            return Ok(self.bump().span);
        }
        if matches!(self.tok(), Tok::Eof) {
            let msg = format!("{} is never closed", &self.src[open.start..open.end]);
            let p =
                Problem::new(Code::ParseUnclosedDelimiter, msg, open).expected(close.describe());
            return Err(self.abort(p));
        }
        Err(self.unexpected(&close.describe()))
    }

    // ---------------------------------------------------------------- names

    fn lower_name(&mut self) -> PResult<Ident> {
        let t = self.tok().clone();
        match t {
            Tok::Lower(_) | Tok::Upper(_) => {
                let tok = self.bump();
                let (Tok::Lower(name) | Tok::Upper(name)) = tok.tok else {
                    return Err(Abort);
                };
                if !is_lower_camel(&name) {
                    let fixed = to_lower_camel(&name);
                    self.push(
                        Problem::new(
                            Code::ParseNameCase,
                            format!("`{name}` must be lowerCamel"),
                            tok.span,
                        )
                        .expected("lowerCamel name")
                        .fix(
                            format!("rename to `{fixed}`"),
                            tok.span,
                            fixed,
                        ),
                    );
                }
                Ok(Ident {
                    name,
                    span: tok.span,
                })
            }
            Tok::Kw(k) => {
                let span = self.span();
                Err(self.abort(Problem::new(
                    Code::ParseReservedWord,
                    format!("`{}` is a reserved word", k.as_str()),
                    span,
                )))
            }
            _ => Err(self.unexpected("a name")),
        }
    }

    fn upper_name(&mut self) -> PResult<Ident> {
        let t = self.tok().clone();
        match t {
            Tok::Lower(_) | Tok::Upper(_) => {
                let tok = self.bump();
                let (Tok::Lower(name) | Tok::Upper(name)) = tok.tok else {
                    return Err(Abort);
                };
                if !is_upper_camel(&name) {
                    let fixed = to_upper_camel(&name);
                    self.push(
                        Problem::new(
                            Code::ParseNameCase,
                            format!("`{name}` must be UpperCamel"),
                            tok.span,
                        )
                        .expected("UpperCamel name")
                        .fix(
                            format!("rename to `{fixed}`"),
                            tok.span,
                            fixed,
                        ),
                    );
                }
                Ok(Ident {
                    name,
                    span: tok.span,
                })
            }
            Tok::Kw(k) => {
                let span = self.span();
                Err(self.abort(Problem::new(
                    Code::ParseReservedWord,
                    format!("`{}` is a reserved word", k.as_str()),
                    span,
                )))
            }
            _ => Err(self.unexpected("a type name")),
        }
    }

    // ---------------------------------------------------------------- recovery

    fn line_start(&self, offset: usize) -> bool {
        let before = &self.src[..offset];
        before
            .rsplit('\n')
            .next()
            .is_some_and(|l| l.chars().all(|c| c == ' '))
    }

    /// Skips to the end of the current line (not consuming the newline), respecting nesting.
    fn skip_line(&mut self) {
        let mut depth: i32 = 0;
        loop {
            let t = &self.toks[self.pos].tok;
            match t {
                Tok::Eof => break,
                Tok::Newline if depth <= 0 => break,
                Tok::RBrace if depth <= 0 => break,
                Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RBrace => depth -= 1,
                _ => {}
            }
            self.pos += 1;
        }
    }

    /// Skips to the next line-initial item keyword (or end of input).
    fn sync_item(&mut self) {
        if !matches!(self.toks[self.pos].tok, Tok::Eof) {
            self.pos += 1;
        }
        loop {
            let t = &self.toks[self.pos];
            if matches!(t.tok, Tok::Eof) {
                break;
            }
            if matches!(
                t.tok,
                Tok::Kw(Kw::Fn | Kw::Type | Kw::Const | Kw::Test | Kw::Extern)
            ) && self.line_start(t.span.start)
            {
                break;
            }
            self.pos += 1;
        }
    }

    // ---------------------------------------------------------------- items

    fn file(&mut self) -> File {
        let mut items = Vec::new();
        loop {
            self.ignore_nl = false;
            self.no_struct = false;
            self.skip_newlines();
            if matches!(self.toks[self.pos].tok, Tok::Eof) {
                break;
            }
            match self.item() {
                Ok(item) => {
                    items.push(item);
                    let t = self.toks[self.pos].tok.clone();
                    if !matches!(t, Tok::Newline | Tok::Eof) {
                        let span = self.toks[self.pos].span;
                        let p = Problem::new(
                            Code::ParseUnexpectedToken,
                            format!("expected end of item, found {}", t.describe()),
                            span,
                        );
                        self.problems.push(p);
                        self.sync_item();
                    }
                }
                Err(Abort) => self.sync_item(),
            }
        }
        File { items }
    }

    fn item(&mut self) -> PResult<Item> {
        match self.tok().clone() {
            Tok::Kw(Kw::Type) => self.type_decl().map(Item::Type),
            Tok::Kw(Kw::Fn | Kw::Extern) => self.fn_decl().map(Item::Fn),
            Tok::Kw(Kw::Const) => self.const_decl().map(Item::Const),
            Tok::Kw(Kw::Test) => self.test_decl().map(Item::Test),
            _ => {
                let span = self.span();
                let msg = format!(
                    "expected `type`, `fn`, `const` or `test`, found {}",
                    self.tok().describe()
                );
                Err(self.abort(
                    Problem::new(Code::ParseBadItem, msg, span)
                        .expected("`type`, `fn`, `const` or `test`"),
                ))
            }
        }
    }

    fn tparams(&mut self) -> PResult<Vec<Ident>> {
        let mut out = Vec::new();
        if self.at(&Tok::LBracket) {
            let open = self.bump().span;
            let saved = std::mem::replace(&mut self.ignore_nl, true);
            loop {
                out.push(self.upper_name()?);
                if !self.eat(&Tok::Comma) || self.at(&Tok::RBracket) {
                    break;
                }
            }
            self.expect_close(&Tok::RBracket, open)?;
            self.ignore_nl = saved;
        }
        Ok(out)
    }

    fn type_decl(&mut self) -> PResult<TypeDecl> {
        let start = self.bump().span; // `type`
        let name = self.upper_name()?;
        let params = self.tparams()?;
        self.expect(&Tok::Eq)?;
        self.skip_newlines();
        let body = self.type_body()?;
        let end = self.prev_end();
        Ok(TypeDecl {
            name,
            params,
            body,
            span: ByteSpan::new(start.start, end),
        })
    }

    fn type_body(&mut self) -> PResult<TypeBody> {
        match self.tok().clone() {
            Tok::LBrace => self.record_body(),
            Tok::Bar => self.variants(),
            Tok::Upper(_) => {
                let next = self.peek_n(1).clone();
                // After a name: `(` starts a payload, `|` continues the variant list.
                let after_nl = self.next_sig(self.pos + 1);
                let continues = matches!(next, Tok::LParen | Tok::Bar)
                    || matches!(self.toks[after_nl].tok, Tok::Bar);
                if continues {
                    self.variants()
                } else {
                    self.type_expr().map(TypeBody::Alias)
                }
            }
            _ => self.type_expr().map(TypeBody::Alias),
        }
    }

    fn record_body(&mut self) -> PResult<TypeBody> {
        let open = self.bump().span;
        let saved = std::mem::replace(&mut self.ignore_nl, true);
        let mut fields = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
            let name = self.lower_name()?;
            self.expect(&Tok::Colon)?;
            let ty = self.type_expr()?;
            fields.push(FieldDecl { name, ty });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect_close(&Tok::RBrace, open)?;
        self.ignore_nl = saved;
        Ok(TypeBody::Record(fields))
    }

    fn variants(&mut self) -> PResult<TypeBody> {
        let mut out = Vec::new();
        self.eat(&Tok::Bar);
        loop {
            self.skip_newlines();
            let name = self.upper_name()?;
            let mut fields = Vec::new();
            let mut end = name.span.end;
            if self.at(&Tok::LParen) {
                let open = self.bump().span;
                let saved = std::mem::replace(&mut self.ignore_nl, true);
                loop {
                    fields.push(self.type_expr()?);
                    if !self.eat(&Tok::Comma) || self.at(&Tok::RParen) {
                        break;
                    }
                }
                end = self.expect_close(&Tok::RParen, open)?.end;
                self.ignore_nl = saved;
            }
            out.push(Variant {
                span: ByteSpan::new(name.span.start, end),
                name,
                fields,
            });
            // `|` may follow on the next line
            let idx = self.next_sig(self.pos);
            if matches!(self.toks[idx].tok, Tok::Bar) {
                self.pos = idx + 1;
            } else {
                break;
            }
        }
        Ok(TypeBody::Adt(out))
    }

    fn fn_decl(&mut self) -> PResult<FnDecl> {
        let start = self.span();
        let mut is_extern = false;
        if self.at_kw(Kw::Extern) {
            let sp = self.bump().span;
            if self.prelude {
                is_extern = true;
            } else {
                self.push(Problem::new(
                    Code::ParseReservedWord,
                    "`extern` is reserved for the prelude",
                    sp,
                ));
            }
        }
        self.expect(&Tok::Kw(Kw::Fn))?;
        let mut module = None;
        if self.prelude && matches!(self.tok(), Tok::Upper(_)) && matches!(self.peek_n(1), Tok::Dot)
        {
            let t = self.bump();
            if let Tok::Upper(n) = t.tok {
                module = Some(Ident {
                    name: n,
                    span: t.span,
                });
            }
            self.bump(); // `.`
        }
        let name = self.lower_name()?;
        let tparams = self.tparams()?;
        let open = self.expect(&Tok::LParen)?;
        let saved = std::mem::replace(&mut self.ignore_nl, true);
        let mut params = Vec::new();
        while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
            let pname = self.lower_name()?;
            self.expect(&Tok::Colon)?;
            let ty = self.type_expr()?;
            params.push(Param { name: pname, ty });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect_close(&Tok::RParen, open)?;
        self.ignore_nl = saved;
        self.expect(&Tok::Arrow)?;
        let ret = self.type_expr()?;

        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        let mut decreases: Option<Expr> = None;
        loop {
            let idx = self.next_sig(self.pos);
            match self.toks[idx].tok {
                Tok::Kw(Kw::Requires | Kw::Ensures | Kw::Decreases) => {
                    self.pos = idx;
                    let kw = self.bump();
                    let e = self.expr()?;
                    match kw.tok {
                        Tok::Kw(Kw::Requires) => requires.push(e),
                        Tok::Kw(Kw::Ensures) => ensures.push(e),
                        _ => {
                            if decreases.is_some() {
                                self.push(Problem::new(
                                    Code::ParseDuplicateClause,
                                    "only one `decreases` clause is allowed",
                                    kw.span,
                                ));
                            } else {
                                decreases = Some(e);
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        let body = if is_extern {
            None
        } else {
            let idx = self.next_sig(self.pos);
            self.pos = idx;
            self.expect(&Tok::Eq)?;
            self.skip_newlines();
            Some(self.expr()?)
        };
        let end = self.prev_end();
        Ok(FnDecl {
            module,
            name,
            tparams,
            params,
            ret,
            requires,
            ensures,
            decreases,
            body,
            is_extern,
            span: ByteSpan::new(start.start, end),
        })
    }

    fn const_decl(&mut self) -> PResult<ConstDecl> {
        let start = self.bump().span;
        let name = self.lower_name()?;
        self.expect(&Tok::Colon)?;
        let ty = self.type_expr()?;
        self.expect(&Tok::Eq)?;
        self.skip_newlines();
        let value = self.expr()?;
        Ok(ConstDecl {
            name,
            ty,
            value,
            span: ByteSpan::new(start.start, self.prev_end()),
        })
    }

    fn test_decl(&mut self) -> PResult<TestDecl> {
        let start = self.bump().span;
        let name = self.lower_name()?;
        self.expect(&Tok::Eq)?;
        self.skip_newlines();
        let body = self.expr()?;
        Ok(TestDecl {
            name,
            body,
            span: ByteSpan::new(start.start, self.prev_end()),
        })
    }

    // ---------------------------------------------------------------- types

    fn type_expr(&mut self) -> PResult<TypeExpr> {
        let mut t = self.atype()?;
        if self.at_kw(Kw::Where) {
            self.bump();
            let pred = self.expr()?;
            t.span = t.span.join(pred.span);
            t.refine = Some(Box::new(pred));
        }
        Ok(t)
    }

    fn atype(&mut self) -> PResult<TypeExpr> {
        match self.tok().clone() {
            Tok::Upper(_) | Tok::Lower(_) => {
                let name = self.upper_name()?;
                let mut args = Vec::new();
                let mut end = name.span.end;
                if self.at(&Tok::LBracket) {
                    let open = self.bump().span;
                    let saved = std::mem::replace(&mut self.ignore_nl, true);
                    loop {
                        args.push(self.type_expr()?);
                        if !self.eat(&Tok::Comma) || self.at(&Tok::RBracket) {
                            break;
                        }
                    }
                    end = self.expect_close(&Tok::RBracket, open)?.end;
                    self.ignore_nl = saved;
                }
                Ok(TypeExpr {
                    span: ByteSpan::new(name.span.start, end),
                    kind: TypeKind::Named { name, args },
                    refine: None,
                })
            }
            Tok::LParen => {
                let open = self.bump().span;
                let saved = std::mem::replace(&mut self.ignore_nl, true);
                let mut elems = Vec::new();
                while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                    elems.push(self.type_expr()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                let close = self.expect_close(&Tok::RParen, open)?;
                self.ignore_nl = saved;
                let span = open.join(close);
                if self.at(&Tok::Arrow) {
                    self.bump();
                    self.skip_newlines();
                    let ret = self.atype()?;
                    let span = span.join(ret.span);
                    return Ok(TypeExpr {
                        kind: TypeKind::Fn(elems, Box::new(ret)),
                        refine: None,
                        span,
                    });
                }
                Ok(match elems.len() {
                    0 => TypeExpr {
                        kind: TypeKind::Unit,
                        refine: None,
                        span,
                    },
                    1 => {
                        let mut only = elems.remove(0);
                        only.span = span;
                        only
                    }
                    _ => TypeExpr {
                        kind: TypeKind::Tuple(elems),
                        refine: None,
                        span,
                    },
                })
            }
            _ => {
                let span = self.span();
                let msg = format!("expected a type, found {}", self.tok().describe());
                Err(self.abort(Problem::new(Code::ParseExpectedType, msg, span).expected("a type")))
            }
        }
    }

    // ---------------------------------------------------------------- expressions

    fn expr(&mut self) -> PResult<Expr> {
        self.or_expr()
    }

    fn mk(kind: ExprKind, span: ByteSpan) -> Expr {
        Expr { kind, span }
    }

    fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
        let span = l.span.join(r.span);
        Self::mk(ExprKind::Binary(op, Box::new(l), Box::new(r)), span)
    }

    /// True when a leading `&&`/`||`/`.` continues the expression on the next line.
    fn continues_with(&mut self, want: &Tok) -> bool {
        if self.at(want) {
            return true;
        }
        if matches!(self.toks[self.pos].tok, Tok::Newline) {
            let idx = self.next_sig(self.pos);
            if &self.toks[idx].tok == want {
                self.pos = idx;
                return true;
            }
        }
        false
    }

    fn or_expr(&mut self) -> PResult<Expr> {
        let mut left = self.and_expr()?;
        while self.continues_with(&Tok::OrOr) {
            self.bump();
            self.skip_newlines();
            let right = self.and_expr()?;
            for side in [&left, &right] {
                if matches!(side.kind, ExprKind::Binary(BinOp::And, ..)) {
                    let sp = side.span;
                    self.push(
                        Problem::new(
                            Code::ParseMixedLogic,
                            "`&&` and `||` need parentheses when mixed",
                            sp,
                        )
                        .fix("parenthesize", ByteSpan::new(sp.start, sp.start), "(")
                        .fix("close the parenthesis", sp.end_point(), ")"),
                    );
                }
            }
            left = Self::bin(BinOp::Or, left, right);
        }
        Ok(left)
    }

    fn and_expr(&mut self) -> PResult<Expr> {
        let mut left = self.cmp_expr()?;
        while self.continues_with(&Tok::AndAnd) {
            self.bump();
            self.skip_newlines();
            let right = self.cmp_expr()?;
            left = Self::bin(BinOp::And, left, right);
        }
        Ok(left)
    }

    fn cmp_expr(&mut self) -> PResult<Expr> {
        let mut left = self.cat_expr()?;
        if let Some(op) = is_cmp(self.tok()) {
            self.bump();
            self.skip_newlines();
            let right = self.cat_expr()?;
            left = Self::bin(op, left, right);
            while let Some(op2) = is_cmp(self.tok()) {
                let sp = self.span();
                self.push(
                    Problem::new(
                        Code::ParseChainedComparison,
                        "comparisons cannot be chained",
                        sp,
                    )
                    .expected("`a < b && b < c`"),
                );
                self.bump();
                self.skip_newlines();
                let r2 = self.cat_expr()?;
                left = Self::bin(op2, left, r2);
            }
        }
        Ok(left)
    }

    fn cat_expr(&mut self) -> PResult<Expr> {
        let mut left = self.add_expr()?;
        while self.at(&Tok::PlusPlus) {
            self.bump();
            self.skip_newlines();
            let right = self.add_expr()?;
            left = Self::bin(BinOp::Concat, left, right);
        }
        Ok(left)
    }

    fn add_expr(&mut self) -> PResult<Expr> {
        let mut left = self.mul_expr()?;
        loop {
            let op = match self.tok() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            self.skip_newlines();
            let right = self.mul_expr()?;
            left = Self::bin(op, left, right);
        }
        Ok(left)
    }

    fn mul_expr(&mut self) -> PResult<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.tok() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Rem,
                _ => break,
            };
            self.bump();
            self.skip_newlines();
            let right = self.unary()?;
            left = Self::bin(op, left, right);
        }
        Ok(left)
    }

    fn unary(&mut self) -> PResult<Expr> {
        let op = match self.tok() {
            Tok::Minus => UnOp::Neg,
            Tok::Bang => UnOp::Not,
            _ => return self.postfix(),
        };
        let start = self.bump().span;
        let operand = self.unary()?;
        let span = start.join(operand.span);
        Ok(Self::mk(ExprKind::Unary(op, Box::new(operand)), span))
    }

    fn args(&mut self, open: ByteSpan) -> PResult<(Vec<Expr>, ByteSpan)> {
        let saved_nl = std::mem::replace(&mut self.ignore_nl, true);
        let saved_ns = std::mem::replace(&mut self.no_struct, false);
        let mut args = Vec::new();
        while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
            args.push(self.expr()?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        let close = self.expect_close(&Tok::RParen, open)?;
        self.ignore_nl = saved_nl;
        self.no_struct = saved_ns;
        Ok((args, close))
    }

    fn field_inits(&mut self) -> PResult<(Vec<FieldInit>, ByteSpan)> {
        let open = self.expect(&Tok::LBrace)?;
        let saved_nl = std::mem::replace(&mut self.ignore_nl, true);
        let saved_ns = std::mem::replace(&mut self.no_struct, false);
        let mut fields = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
            let name = self.lower_name()?;
            self.expect(&Tok::Colon)?;
            let value = self.expr()?;
            fields.push(FieldInit { name, value });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        let close = self.expect_close(&Tok::RBrace, open)?;
        self.ignore_nl = saved_nl;
        self.no_struct = saved_ns;
        Ok((fields, open.join(close)))
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            // leading `.` on the next line continues a method chain
            let dot = self.continues_with(&Tok::Dot);
            if dot {
                self.bump();
                match self.tok().clone() {
                    Tok::Int(n) => {
                        let t = self.bump();
                        let idx = n.parse::<u32>().unwrap_or(u32::MAX);
                        let span = e.span.join(t.span);
                        e = Self::mk(ExprKind::TupleField(Box::new(e), idx), span);
                    }
                    Tok::Lower(_) | Tok::Upper(_) => {
                        let name = self.lower_name()?;
                        if self.at(&Tok::LParen) {
                            let open = self.bump().span;
                            let (args, close) = self.args(open)?;
                            let span = e.span.join(close);
                            e = Self::mk(
                                ExprKind::MethodCall {
                                    recv: Box::new(e),
                                    name,
                                    args,
                                },
                                span,
                            );
                        } else {
                            let span = e.span.join(name.span);
                            e = Self::mk(ExprKind::Field(Box::new(e), name), span);
                        }
                    }
                    _ => return Err(self.unexpected("a field or method name")),
                }
                continue;
            }
            match self.tok() {
                Tok::LParen => {
                    let open = self.bump().span;
                    let (args, close) = self.args(open)?;
                    let span = e.span.join(close);
                    e = Self::mk(
                        ExprKind::Call {
                            callee: Box::new(e),
                            args,
                        },
                        span,
                    );
                }
                Tok::LBracket => {
                    let open = self.bump().span;
                    let saved_nl = std::mem::replace(&mut self.ignore_nl, true);
                    let saved_ns = std::mem::replace(&mut self.no_struct, false);
                    let idx = self.expr()?;
                    let close = self.expect_close(&Tok::RBracket, open)?;
                    self.ignore_nl = saved_nl;
                    self.no_struct = saved_ns;
                    let span = e.span.join(close);
                    e = Self::mk(ExprKind::Index(Box::new(e), Box::new(idx)), span);
                }
                Tok::Kw(Kw::With) => {
                    self.bump();
                    let (fields, braces) = self.field_inits()?;
                    let span = e.span.join(braces);
                    e = Self::mk(
                        ExprKind::With {
                            base: Box::new(e),
                            fields,
                        },
                        span,
                    );
                }
                _ => break,
            }
        }
        Ok(e)
    }

    /// True if the tokens at the current `(` form lambda parameters (`(..) =>`).
    fn paren_is_lambda(&self) -> bool {
        let mut depth = 0usize;
        let mut i = self.pos;
        while i < self.toks.len() {
            match self.toks[i].tok {
                Tok::LParen => depth += 1,
                Tok::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        let j = self.next_sig(i + 1);
                        return matches!(self.toks[j].tok, Tok::FatArrow);
                    }
                }
                Tok::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    /// True if `Name {` begins a record literal (`{ field :`).
    fn brace_is_record(&self, brace_idx: usize) -> bool {
        let a = self.next_sig(brace_idx + 1);
        let b = self.next_sig(a + 1);
        matches!(self.toks[a].tok, Tok::Lower(_)) && matches!(self.toks[b].tok, Tok::Colon)
    }

    fn primary(&mut self) -> PResult<Expr> {
        let t = self.tok().clone();
        let span = self.span();
        match t {
            Tok::Int(s) => {
                self.bump();
                Ok(Self::mk(ExprKind::Int(s), span))
            }
            Tok::Float(s) => {
                self.bump();
                Ok(Self::mk(ExprKind::Float(s), span))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Self::mk(ExprKind::Str(s), span))
            }
            Tok::Kw(Kw::True) => {
                self.bump();
                Ok(Self::mk(ExprKind::Bool(true), span))
            }
            Tok::Kw(Kw::False) => {
                self.bump();
                Ok(Self::mk(ExprKind::Bool(false), span))
            }
            Tok::LParen => self.paren(),
            Tok::LBracket => {
                let open = self.bump().span;
                let saved_nl = std::mem::replace(&mut self.ignore_nl, true);
                let saved_ns = std::mem::replace(&mut self.no_struct, false);
                let mut items = Vec::new();
                while !self.at(&Tok::RBracket) && !self.at(&Tok::Eof) {
                    items.push(self.expr()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                let close = self.expect_close(&Tok::RBracket, open)?;
                self.ignore_nl = saved_nl;
                self.no_struct = saved_ns;
                Ok(Self::mk(ExprKind::List(items), open.join(close)))
            }
            Tok::LBrace => self.block(),
            Tok::Kw(Kw::If) => self.if_expr(),
            Tok::Kw(Kw::Match) => self.match_expr(),
            Tok::Underscore if !self.no_struct && matches!(self.peek_n(1), Tok::FatArrow) => {
                let p = self.bump();
                let param = LambdaParam {
                    name: Ident {
                        name: "_".into(),
                        span: p.span,
                    },
                    ty: None,
                };
                self.lambda_body(vec![param], span)
            }
            Tok::Lower(name) => {
                if !self.no_struct && matches!(self.peek_n(1), Tok::FatArrow) {
                    let p = self.bump();
                    let param = LambdaParam {
                        name: Ident { name, span: p.span },
                        ty: None,
                    };
                    return self.lambda_body(vec![param], span);
                }
                self.bump();
                Ok(Self::mk(ExprKind::Var(Ident { name, span }), span))
            }
            Tok::Upper(name) => {
                self.bump();
                let id = Ident { name, span };
                if matches!(self.tok(), Tok::Dot) && matches!(self.peek_n(1), Tok::Lower(_)) {
                    self.bump();
                    let member = self.lower_name()?;
                    let sp = span.join(member.span);
                    return Ok(Self::mk(
                        ExprKind::Path {
                            module: id,
                            name: member,
                        },
                        sp,
                    ));
                }
                if matches!(self.tok(), Tok::LBrace)
                    && !self.no_struct
                    && self.brace_is_record(self.pos)
                {
                    let (fields, braces) = self.field_inits()?;
                    return Ok(Self::mk(
                        ExprKind::Record { ty: id, fields },
                        span.join(braces),
                    ));
                }
                Ok(Self::mk(ExprKind::Ctor(id), span))
            }
            _ => {
                let msg = format!("expected an expression, found {}", t.describe());
                if matches!(t, Tok::Eof) {
                    return Err(self.unexpected("an expression"));
                }
                Err(self.abort(
                    Problem::new(Code::ParseExpectedExpr, msg, span).expected("an expression"),
                ))
            }
        }
    }

    fn lambda_body(&mut self, params: Vec<LambdaParam>, start: ByteSpan) -> PResult<Expr> {
        self.expect(&Tok::FatArrow)?;
        self.skip_newlines();
        let body = self.expr()?;
        let span = start.join(body.span);
        Ok(Self::mk(
            ExprKind::Lambda {
                params,
                body: Box::new(body),
            },
            span,
        ))
    }

    fn paren(&mut self) -> PResult<Expr> {
        let open = self.span();
        let is_lambda = self.paren_is_lambda();
        self.bump();
        let saved_nl = std::mem::replace(&mut self.ignore_nl, true);
        let saved_ns = std::mem::replace(&mut self.no_struct, false);
        if is_lambda {
            let mut params = Vec::new();
            while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                let name = if self.at(&Tok::Underscore) {
                    let t = self.bump();
                    Ident {
                        name: "_".into(),
                        span: t.span,
                    }
                } else {
                    self.lower_name()?
                };
                let ty = if self.eat(&Tok::Colon) {
                    Some(self.type_expr()?)
                } else {
                    None
                };
                params.push(LambdaParam { name, ty });
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect_close(&Tok::RParen, open)?;
            self.ignore_nl = saved_nl;
            self.no_struct = saved_ns;
            return self.lambda_body(params, open);
        }
        if self.at(&Tok::RParen) {
            let close = self.bump().span;
            self.ignore_nl = saved_nl;
            self.no_struct = saved_ns;
            return Ok(Self::mk(ExprKind::Unit, open.join(close)));
        }
        let first = self.expr()?;
        if self.at(&Tok::Comma) {
            let mut elems = vec![first];
            while self.eat(&Tok::Comma) {
                if self.at(&Tok::RParen) {
                    break;
                }
                elems.push(self.expr()?);
            }
            let close = self.expect_close(&Tok::RParen, open)?;
            self.ignore_nl = saved_nl;
            self.no_struct = saved_ns;
            return Ok(Self::mk(ExprKind::Tuple(elems), open.join(close)));
        }
        let close = self.expect_close(&Tok::RParen, open)?;
        self.ignore_nl = saved_nl;
        self.no_struct = saved_ns;
        Ok(Self::mk(ExprKind::Paren(Box::new(first)), open.join(close)))
    }

    fn if_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        let saved_ns = std::mem::replace(&mut self.no_struct, true);
        let cond = self.expr();
        self.no_struct = saved_ns;
        let cond = cond?;
        if !self.at(&Tok::LBrace) {
            return Err(self.unexpected("`{`"));
        }
        let then = self.block()?;
        if self.at_kw(Kw::Else) {
            self.bump();
            let els = if self.at_kw(Kw::If) {
                self.if_expr()?
            } else {
                self.block()?
            };
            let span = start.join(els.span);
            return Ok(Self::mk(
                ExprKind::If {
                    cond: Box::new(cond),
                    then: Box::new(then),
                    els: Some(Box::new(els)),
                },
                span,
            ));
        }
        let span = start.join(then.span);
        self.push(
            Problem::new(Code::ParseElseRequired, "`if` needs an `else` branch", span)
                .expected("`else { .. }` on the same line as the closing `}`"),
        );
        Ok(Self::mk(
            ExprKind::If {
                cond: Box::new(cond),
                then: Box::new(then),
                els: None,
            },
            span,
        ))
    }

    fn match_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        let saved_ns = std::mem::replace(&mut self.no_struct, true);
        let scrut = self.expr();
        self.no_struct = saved_ns;
        let scrut = scrut?;
        let open = self.expect(&Tok::LBrace)?;
        let saved_nl = std::mem::replace(&mut self.ignore_nl, false);
        let saved_ns = std::mem::replace(&mut self.no_struct, false);
        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&Tok::RBrace) || self.at(&Tok::Eof) {
                break;
            }
            match self.arm() {
                Ok(arm) => {
                    arms.push(arm);
                    self.arm_separator();
                }
                Err(Abort) => {
                    self.skip_line();
                    self.arm_separator();
                }
            }
        }
        let close = self.expect_close(&Tok::RBrace, open)?;
        self.ignore_nl = saved_nl;
        self.no_struct = saved_ns;
        Ok(Self::mk(
            ExprKind::Match {
                scrut: Box::new(scrut),
                arms,
            },
            start.join(close),
        ))
    }

    /// After an arm: a line break or `}` must follow; `,` / `;` are reported and skipped.
    fn arm_separator(&mut self) {
        match self.toks[self.pos].tok {
            Tok::Newline | Tok::RBrace | Tok::Eof => {}
            Tok::Comma | Tok::Semi => {
                let sp = self.toks[self.pos].span;
                self.push(
                    Problem::new(
                        Code::ParseStraySeparator,
                        "arms are separated by line breaks, not `,`",
                        sp,
                    )
                    .fix("delete it", sp, ""),
                );
                self.pos += 1;
            }
            _ => {
                let sp = self.toks[self.pos].span;
                let msg = format!(
                    "expected a line break after the arm, found {}",
                    self.toks[self.pos].tok.describe()
                );
                self.push(
                    Problem::new(Code::ParseUnexpectedToken, msg, sp).expected("a line break"),
                );
                self.skip_line();
            }
        }
    }

    fn arm(&mut self) -> PResult<Arm> {
        let pat = self.pattern()?;
        let mut guard = None;
        if self.at_kw(Kw::If) {
            self.bump();
            let saved_ns = std::mem::replace(&mut self.no_struct, true);
            let g = self.expr();
            self.no_struct = saved_ns;
            guard = Some(g?);
        }
        self.expect(&Tok::FatArrow)?;
        self.skip_newlines();
        let body = self.expr()?;
        let span = pat.span.join(body.span);
        Ok(Arm {
            pat,
            guard,
            body,
            span,
        })
    }

    fn block(&mut self) -> PResult<Expr> {
        let open = self.expect(&Tok::LBrace)?;
        let saved_nl = std::mem::replace(&mut self.ignore_nl, false);
        let saved_ns = std::mem::replace(&mut self.no_struct, false);
        let mut stmts = Vec::new();
        let mut tail: Option<Expr> = None;
        let mut had_error = false;
        loop {
            self.skip_newlines();
            if self.at(&Tok::RBrace) || self.at(&Tok::Eof) {
                break;
            }
            let r: PResult<()> = (|| {
                match self.tok().clone() {
                    Tok::Kw(Kw::Let) => stmts.push(self.let_stmt()?),
                    Tok::Kw(Kw::Assert) => {
                        let start = self.bump().span;
                        let cond = self.expr()?;
                        let span = start.join(cond.span);
                        stmts.push(Stmt::Assert { cond, span });
                    }
                    _ => {
                        let e = self.expr()?;
                        if let Some(prev) = tail.replace(e) {
                            self.push(
                                Problem::new(
                                    Code::ParseUnexpectedToken,
                                    "this expression is discarded; bind it with `let` or remove it",
                                    prev.span,
                                )
                                .expected("`let`, `assert` or the final expression"),
                            );
                        }
                        return Ok(());
                    }
                }
                Ok(())
            })();
            if r.is_err() {
                had_error = true;
                self.skip_line();
                continue;
            }
            match self.toks[self.pos].tok {
                Tok::Newline | Tok::RBrace | Tok::Eof => {}
                Tok::Semi => {
                    let sp = self.toks[self.pos].span;
                    self.push(
                        Problem::new(
                            Code::ParseStraySeparator,
                            "statements are separated by line breaks, not `;`",
                            sp,
                        )
                        .fix("delete it", sp, ""),
                    );
                    self.pos += 1;
                }
                _ => {
                    let sp = self.toks[self.pos].span;
                    let msg = format!(
                        "expected a line break, found {}",
                        self.toks[self.pos].tok.describe()
                    );
                    self.push(
                        Problem::new(Code::ParseUnexpectedToken, msg, sp).expected("a line break"),
                    );
                    had_error = true;
                    self.skip_line();
                }
            }
            // an expression followed by further statements was not the tail
            if tail.is_some()
                && !matches!(
                    self.toks[self.next_sig(self.pos)].tok,
                    Tok::RBrace | Tok::Eof
                )
                && let Some(prev) = tail.take()
            {
                self.push(
                    Problem::new(
                        Code::ParseUnexpectedToken,
                        "this expression is discarded; bind it with `let` or remove it",
                        prev.span,
                    )
                    .expected("`let`, `assert` or the final expression"),
                );
            }
        }
        let close = self.expect_close(&Tok::RBrace, open)?;
        self.ignore_nl = saved_nl;
        self.no_struct = saved_ns;
        let span = open.join(close);
        let tail = match tail {
            Some(t) => t,
            None => {
                if !had_error {
                    self.push(
                        Problem::new(
                            Code::ParseBlockNoTail,
                            "a block must end with an expression",
                            span,
                        )
                        .expected("the block's result expression"),
                    );
                }
                Self::mk(ExprKind::Unit, close)
            }
        };
        Ok(Self::mk(
            ExprKind::Block {
                stmts,
                tail: Box::new(tail),
            },
            span,
        ))
    }

    fn let_stmt(&mut self) -> PResult<Stmt> {
        let start = self.bump().span;
        let pat = self.pattern()?;
        let ty = if self.eat(&Tok::Colon) {
            Some(self.type_expr()?)
        } else {
            None
        };
        self.expect(&Tok::Eq)?;
        self.skip_newlines();
        let init = self.expr()?;
        let span = start.join(init.span);
        Ok(Stmt::Let {
            pat,
            ty,
            init,
            span,
        })
    }

    // ---------------------------------------------------------------- patterns

    fn pattern(&mut self) -> PResult<Pat> {
        let span = self.span();
        match self.tok().clone() {
            Tok::Underscore => {
                self.bump();
                Ok(Pat {
                    kind: PatKind::Wild,
                    span,
                })
            }
            Tok::Lower(_) => {
                let id = self.lower_name()?;
                Ok(Pat {
                    span: id.span,
                    kind: PatKind::Bind(id),
                })
            }
            Tok::Int(s) => {
                self.bump();
                Ok(Pat {
                    kind: PatKind::Lit(Lit::Int(s)),
                    span,
                })
            }
            Tok::Float(s) => {
                self.bump();
                Ok(Pat {
                    kind: PatKind::Lit(Lit::Float(s)),
                    span,
                })
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Pat {
                    kind: PatKind::Lit(Lit::Str(s)),
                    span,
                })
            }
            Tok::Kw(Kw::True) => {
                self.bump();
                Ok(Pat {
                    kind: PatKind::Lit(Lit::Bool(true)),
                    span,
                })
            }
            Tok::Kw(Kw::False) => {
                self.bump();
                Ok(Pat {
                    kind: PatKind::Lit(Lit::Bool(false)),
                    span,
                })
            }
            Tok::Minus => {
                self.bump();
                match self.tok().clone() {
                    Tok::Int(s) => {
                        let t = self.bump();
                        Ok(Pat {
                            kind: PatKind::Lit(Lit::Int(format!("-{s}"))),
                            span: span.join(t.span),
                        })
                    }
                    Tok::Float(s) => {
                        let t = self.bump();
                        Ok(Pat {
                            kind: PatKind::Lit(Lit::Float(format!("-{s}"))),
                            span: span.join(t.span),
                        })
                    }
                    _ => Err(self.unexpected("a number")),
                }
            }
            Tok::Upper(_) => {
                let name = self.upper_name()?;
                let mut args = Vec::new();
                let mut end = name.span.end;
                if self.at(&Tok::LParen) {
                    let open = self.bump().span;
                    let saved = std::mem::replace(&mut self.ignore_nl, true);
                    while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                        args.push(self.pattern()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    end = self.expect_close(&Tok::RParen, open)?.end;
                    self.ignore_nl = saved;
                }
                Ok(Pat {
                    span: ByteSpan::new(name.span.start, end),
                    kind: PatKind::Ctor { name, args },
                })
            }
            Tok::LParen => {
                let open = self.bump().span;
                let saved = std::mem::replace(&mut self.ignore_nl, true);
                let mut elems = Vec::new();
                let mut saw_comma = false;
                while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                    elems.push(self.pattern()?);
                    if self.eat(&Tok::Comma) {
                        saw_comma = true;
                        if elems.len() == 1 && self.at(&Tok::RParen) {
                            // `(p,)` is not a tuple: tuples have at least two elements
                            return Err(self.unexpected("a second tuple element"));
                        }
                    } else {
                        break;
                    }
                }
                let close = self.expect_close(&Tok::RParen, open)?;
                self.ignore_nl = saved;
                let span = open.join(close);
                Ok(match (elems.len(), saw_comma) {
                    (0, _) => Pat {
                        kind: PatKind::Unit,
                        span,
                    },
                    (1, false) => {
                        let mut only = elems.remove(0);
                        only.span = span;
                        only
                    }
                    _ => Pat {
                        kind: PatKind::Tuple(elems),
                        span,
                    },
                })
            }
            Tok::LBracket => {
                let open = self.bump().span;
                let saved = std::mem::replace(&mut self.ignore_nl, true);
                let mut items = Vec::new();
                let mut rest = None;
                while !self.at(&Tok::RBracket) && !self.at(&Tok::Eof) {
                    if self.at(&Tok::DotDot) {
                        self.bump();
                        rest = Some(if matches!(self.tok(), Tok::Lower(_)) {
                            Some(self.lower_name()?)
                        } else {
                            None
                        });
                        self.eat(&Tok::Comma);
                        break;
                    }
                    items.push(self.pattern()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                let close = self.expect_close(&Tok::RBracket, open)?;
                self.ignore_nl = saved;
                Ok(Pat {
                    kind: PatKind::List { items, rest },
                    span: open.join(close),
                })
            }
            _ => {
                let msg = format!("expected a pattern, found {}", self.tok().describe());
                if matches!(self.tok(), Tok::Eof) {
                    return Err(self.unexpected("a pattern"));
                }
                Err(self.abort(
                    Problem::new(Code::ParseExpectedPattern, msg, span).expected("a pattern"),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests;

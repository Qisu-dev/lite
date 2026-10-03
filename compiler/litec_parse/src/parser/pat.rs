use crate::parser::{Expected, ParseCtx, trait_::Parse};
use litec_ast::{
    TokenKind,
    ast::{Expr, Ident, Mutability, Pat, PatKind, Path, RangeLimits, StructFieldPat},
    tok,
    util::Precedence,
};
use litec_error::PResult;
use litec_span::{Span, Spanned};

impl Parse for Pat {
    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        ctx.parse_pat_or()
    }
}

impl<'a, 'src> ParseCtx<'a, 'src> {
    fn parse_pat_or(&mut self) -> PResult<Pat> {
        let start = self.current_span();

        // 前导 `|` 可选——支持 `| A | B` 与多行写法
        self.eat(tok!(|));

        let first = self.parse_pat_atom()?;
        if !self.check(tok!(|)) {
            return Ok(first);
        }

        let mut pats = vec![first];
        while self.eat(tok!(|)) {
            pats.push(self.parse_pat_atom()?);
        }
        let span = pats.first().unwrap().span.extend(pats.last().unwrap().span);
        Ok(self.mk_pat(PatKind::Or(pats), span))
    }

    fn parse_pat_atom(&mut self) -> PResult<Pat> {
        let start = self.current_span();

        match self.peek_kind() {
            tok!(_) => {
                self.bump();
                Ok(self.mk_pat(PatKind::Wild, start))
            }

            tok!(mut) => {
                self.bump();
                let ident: Ident = self.parse()?;
                let span = start.extend(ident.span);
                Ok(self.mk_pat(PatKind::Ident(Mutability::Mut, ident), span))
            }

            // `..` —— Rest 或前缀 range
            tok!(..) => {
                if self.peek(1).kind.is_pat_expr_start() {
                    self.parse_pat_range_prefix(start)
                } else {
                    self.bump();
                    Ok(self.mk_pat(PatKind::Rest, start))
                }
            }

            // `..=` —— 前缀 range（必须跟 end）
            tok!(..=) => self.parse_pat_range_prefix(start),

            tok!(OpenParen) => self.parse_pat_tuple(start),
            tok!(OpenBracket) => self.parse_pat_slice(start),

            tok!(-) | TokenKind::Literal { .. } => self.parse_pat_expr_or_range(start),

            TokenKind::Ident
            | TokenKind::SelfLower
            | TokenKind::SelfUpper
            | TokenKind::Crate
            | TokenKind::Super
            | TokenKind::PathAccess => self.parse_pat_path(start),

            _ => Err(self.unexpected(&[Expected::Pattern])),
        }
    }

    fn parse_pat_tuple(&mut self, start: Span) -> PResult<Pat> {
        self.expect(tok!(OpenParen))?;
        let mut elems = Vec::new();
        while !self.check(tok!(CloseParen)) && !self.at_eof() {
            elems.push(self.parse_pat_or()?);
            if !self.eat(tok!(,)) {
                break;
            }
        }
        let close = self.expect(tok!(CloseParen))?;
        Ok(self.mk_pat(PatKind::Tuple(elems), start.extend(close.span)))
    }

    fn parse_pat_slice(&mut self, start: Span) -> PResult<Pat> {
        self.expect(tok!(OpenBracket))?;
        let mut elems = Vec::new();
        while !self.check(tok!(CloseBracket)) && !self.at_eof() {
            elems.push(self.parse_pat_or()?);
            if !self.eat(tok!(,)) {
                break;
            }
        }
        let close = self.expect(tok!(CloseBracket))?;
        Ok(self.mk_pat(PatKind::Slice(elems), start.extend(close.span)))
    }

    fn parse_pat_path(&mut self, start: Span) -> PResult<Pat> {
        let path: Path = self.parse()?;

        // `Point { x, y }`
        if self.check(tok!(OpenBrace)) {
            return self.parse_pat_struct(start, path);
        }

        // `Some(x)` / `Point(x, y)`
        if self.check(tok!(OpenParen)) {
            return self.parse_pat_enum_tuple(start, path);
        }

        // 单 ident、无泛型参数——可能是绑定
        if path.segments.len() == 1 && path.segments[0].generic_args.is_none() {
            let segment = &path.segments[0];
            let name = segment.name.clone();
            let span = start.extend(segment.span);
            return Ok(self.mk_pat(PatKind::Ident(Mutability::Immut, name), span));
        }

        // 复杂路径 / 大写 ident——语义层消歧
        let span = path.span;
        Ok(self.mk_pat(PatKind::Enum(path, None), span))
    }

    fn parse_pat_enum_tuple(&mut self, start: Span, path: Path) -> PResult<Pat> {
        self.expect(tok!(OpenParen))?;
        let mut elems = Vec::new();
        while !self.check(tok!(CloseParen)) && !self.at_eof() {
            elems.push(self.parse_pat_or()?);
            if !self.eat(tok!(,)) {
                break;
            }
        }
        let close = self.expect(tok!(CloseParen))?;
        let span = start.extend(close.span);

        let payload = match elems.len() {
            0 => None,
            1 => Some(Box::new(elems.into_iter().next().unwrap())),
            _ => {
                let tuple_span = elems[0].span.extend(elems[elems.len() - 1].span);
                let tuple = self.mk_pat(PatKind::Tuple(elems), tuple_span);
                Some(Box::new(tuple))
            }
        };

        Ok(self.mk_pat(PatKind::Enum(path, payload), span))
    }

    fn parse_pat_struct(&mut self, start: Span, path: Path) -> PResult<Pat> {
        self.expect(tok!(OpenBrace))?;

        let mut fields = Vec::new();
        let mut has_rest = false;

        while !self.check(tok!(CloseBrace)) && !self.at_eof() {
            if self.eat(tok!(..)) {
                has_rest = true;
                break;
            }

            let field_start = self.current_span();
            let name: Ident = self.parse()?;

            // `x` 简写或 `x: pat`
            let pat = if self.eat(tok!(:)) {
                self.parse_pat_or()?
            } else {
                let span = name.span;
                self.mk_pat(PatKind::Ident(Mutability::Immut, name.clone()), span)
            };

            let field_span = field_start.extend(pat.span);
            fields.push(StructFieldPat {
                node_id: self.node_id(),
                name,
                pat,
                span: field_span,
            });

            if !self.eat(tok!(,)) {
                break;
            }
        }

        let close = self.expect(tok!(CloseBrace))?;
        Ok(self.mk_pat(
            PatKind::Struct(path, fields, has_rest),
            start.extend(close.span),
        ))
    }

    /// 前缀 range：`..end` / `..=end`
    fn parse_pat_range_prefix(&mut self, start: Span) -> PResult<Pat> {
        let limits = self.parse_range_limits()?;
        let end = Some(Box::new(self.parse_pat_expr()?));
        let span = start.extend(end.as_ref().unwrap().span);
        Ok(self.mk_pat(PatKind::Range(None, end, limits), span))
    }

    /// 表达式模式——可能是 `0`，也可能是 `1..=5`
    fn parse_pat_expr_or_range(&mut self, start: Span) -> PResult<Pat> {
        let first = self.parse_pat_expr()?;

        // 中缀 / 后缀 range
        if self.check(tok!(..)) || self.check(tok!(..=)) {
            let limits = self.parse_range_limits()?;
            let end = self.parse_opt_pat_range_end()?;
            let span = match &end {
                Some(e) => start.extend(e.span),
                None => start.extend(limits.span),
            };
            return Ok(self.mk_pat(PatKind::Range(Some(Box::new(first)), end, limits), span));
        }

        // 普通表达式模式
        let span = first.span;
        Ok(self.mk_pat(PatKind::Expr(Box::new(first)), span))
    }

    fn parse_opt_pat_range_end(&mut self) -> PResult<Option<Box<Expr>>> {
        let mark = self.mark();
        match self.parse_pat_expr() {
            Ok(e) => Ok(Some(Box::new(e))),
            Err(e) => {
                if self.cursor != mark.cursor {
                    Err(e)
                } else {
                    self.restore(mark);
                    Ok(None)
                }
            }
        }
    }

    fn parse_range_limits(&mut self) -> PResult<Spanned<RangeLimits>> {
        let tok = self.peek(0);
        let limits = match tok.kind {
            tok!(..) => RangeLimits::HalfOpen,
            tok!(..=) => RangeLimits::Closed,
            _ => {
                return Err(
                    self.unexpected(&[Expected::Exact(tok!(..)), Expected::Exact(tok!(..=))])
                );
            }
        };
        let span = tok.span;
        self.bump();
        Ok(Spanned::<RangeLimits>::new(limits, span))
    }

    fn parse_pat_expr(&mut self) -> PResult<Expr> {
        self.parse_expr_with_precedence(Precedence::BitOr.next())
    }
}

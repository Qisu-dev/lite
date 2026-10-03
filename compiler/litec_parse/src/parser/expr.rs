use litec_ast::ast::{
    Arm, AssignOp, BinOp, Block, Expr, ExprKind, GenericArgs, Ident, Mutability, Pat, Path, PathSegment, QSelf, Ty, UnOp,
};
use litec_ast::token::{Lit, LiteralKind};
use litec_ast::util::{AssocOp, Fixity, Precedence};
use litec_ast::{TokenKind, tok, token};
use litec_error::{Diag, PResult};
use litec_span::Span;

use crate::parser::{Expected, ParseContext};

use super::ParseCtx;
use super::trait_::Parse;

impl Parse for Ident {
    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        let token = ctx.expect(tok!(Ident))?;
        Ok(Ident::new(token.text, token.span))
    }
}

impl Parse for PathSegment {
    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        let ident: Ident = ctx.parse()?;

        let mark = ctx.mark();

        if (ctx.current_context() != ParseContext::Expr && ctx.check(tok!(<)))
            || (ctx.eat(tok!(::)) && ctx.check(tok!(<)))
        {
            let generic_args: GenericArgs = ctx.parse()?;

            let span = ident.span.extend(generic_args.span);
            return Ok(PathSegment {
                node_id: ctx.node_id(),
                name: ident,
                span,
                generic_args: Some(generic_args),
            });
        }

        ctx.restore(mark);
        let span = ident.span;
        Ok(PathSegment {
            node_id: ctx.node_id(),
            name: ident,
            span,
            generic_args: None,
        })
    }
}

impl Parse for Path {
    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        let start = ctx.current_span();

        // 前导 `<` —— qself
        if ctx.check(tok!(<)) {
            return ctx.parse_qpath(start);
        }

        // 普通路径
        let mut segments = vec![ctx.parse::<PathSegment>()?];
        while ctx.eat(tok!(::)) {
            segments.push(ctx.parse::<PathSegment>()?);
        }
        let span = start.extend(segments.last().unwrap().span);

        Ok(Path {
            node_id: ctx.node_id(),
            segments,
            span,
            qself: None,
        })
    }
}

impl<'a, 'src> ParseCtx<'a, 'src> {
    /// `<Ty as Trait>::segment::...`
    fn parse_qpath(&mut self, start: Span) -> PResult<Path> {
        self.expect(tok!(<))?;
        let ty: Ty = self.parse()?;
        self.expect(tok!(as))?;
        let trait_: Path = self.parse()?;
        self.expect(tok!(>))?;
        self.expect(tok!(::))?;

        // qself 后的 segments 是 Type 上下文，允许裸 `<` 泛型
        let segments = self.with_context(ParseContext::Type, |ctx| {
            let mut segs = vec![ctx.parse::<PathSegment>()?];
            while ctx.eat(tok!(::)) {
                segs.push(ctx.parse::<PathSegment>()?);
            }
            Ok(segs)
        })?;

        let span = start.extend(segments.last().unwrap().span);

        Ok(Path {
            node_id: self.node_id(),
            segments,
            span,
            qself: Some(Box::new(QSelf {
                ty: Box::new(ty),
                trait_,
            })),
        })
    }
}

impl Parse for token::Lit {
    fn parse(ctx: &mut ParseCtx) -> PResult<Self> {
        let literal = ctx.expect_literal()?;
        let lit = match literal.kind {
            TokenKind::Literal { kind, suffix } => token::Lit {
                kind,
                value: literal.text,
                suffix,
                span: literal.span,
            },
            _ => unreachable!(),
        };
        Ok(lit)
    }
}

impl Parse for Expr {
    fn parse(ctx: &mut ParseCtx) -> PResult<Self> {
        ctx.parse_expr()
    }
}

impl<'a, 'src> ParseCtx<'a, 'src> {
    #[inline]
    pub(crate) fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_expr_with_precedence(Precedence::Jump)
    }

    pub(crate) fn parse_expr_with_precedence(&mut self, min_bp: Precedence) -> PResult<Expr> {
        let mut lhs = self.parse_prefix()?;
        lhs = self.parse_postfix(lhs)?;

        loop {
            let op = match AssocOp::from_token(self.peek(0)) {
                Some(op) => op,
                None => break,
            };
            let op_span = self.peek(0).span;

            if matches!(op, AssocOp::Cast) {
                let p = Precedence::Cast;
                if p < min_bp {
                    break;
                }
                self.bump();
                let ty: Ty = self.parse()?;
                let span = lhs.span.extend(ty.span);
                lhs = self.mk_expr(ExprKind::Cast(Box::new(lhs), Box::new(ty)), span);
                lhs = self.parse_postfix(lhs)?;
                continue;
            }

            // Range —— TODO: 三种形态，暂不支持
            if matches!(op, AssocOp::Range(_)) {
                break;
            }

            let p = op.precedence();
            let (l_bp, r_bp) = match op.fixity() {
                Fixity::Left | Fixity::None => (p, p.next()),
                Fixity::Right => (p, p),
            };
            if l_bp < min_bp {
                break;
            }

            self.bump();
            let rhs = self.parse_expr_with_precedence(r_bp)?;
            let span = lhs.span.extend(rhs.span);

            lhs = match op {
                AssocOp::Binary(bin_op) => self.mk_expr(
                    ExprKind::Binary(Box::new(lhs), BinOp::new(bin_op, span), Box::new(rhs)),
                    span,
                ),
                AssocOp::Assign => {
                    self.mk_expr(ExprKind::Assign(Box::new(lhs), Box::new(rhs)), span)
                }
                AssocOp::AssignOp(assign_op) => self.mk_expr(
                    ExprKind::AssignOp(
                        Box::new(lhs),
                        AssignOp::new(assign_op, op_span),
                        Box::new(rhs),
                    ),
                    span,
                ),
                AssocOp::Cast | AssocOp::Range(_) => unreachable!("handled above"),
            };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> PResult<Expr> {
        let span = self.current_span();

        match self.peek_kind() {
            TokenKind::Literal { .. } => {
                let lit: Lit = self.parse()?;
                let span = lit.span;
                Ok(self.mk_expr(ExprKind::Literal(lit), span))
            }

            TokenKind::Ident
            | TokenKind::SelfLower
            | TokenKind::SelfUpper
            | TokenKind::Crate
            | TokenKind::Super
            | TokenKind::PathAccess => {
                let path: Path = self.parse()?;
                let span = path.span;
                Ok(self.mk_expr(ExprKind::Path(path), span))
            }

            tok!(OpenParen) => self.parse_paren_or_tuple(),
            tok!(OpenBracket) => self.parse_array_literal(),
            tok!(OpenBrace) => self.parse_block_expr(),

            tok!(if) => self.parse_if_expr(),
            tok!(match) => self.parse_match_expr(),
            tok!(loop) => self.parse_loop_expr(),
            tok!(while) => self.parse_while_expr(),
            tok!(for) => self.parse_for_expr(),

            tok!(return) => self.parse_return_expr(),
            tok!(break) => self.parse_break_expr(),
            tok!(continue) => {
                self.bump();
                Ok(self.mk_expr(ExprKind::Continue, span))
            }
            tok!(let) => {
                if !self.allow_let_expr {
                    return Err(self.unexpected(&[Expected::Expr]));
                }
                self.parse_let_expr(span)
            }

            tok!(-) => self.parse_unary(UnOp::Neg, span),
            tok!(!) => self.parse_unary(UnOp::Not, span),
            tok!(&) => self.parse_ref_expr(Mutability::Immut, span),
            tok!(mut) => {
                self.bump();
                self.parse_ref_expr(Mutability::Mut, span)
            }
            tok!(*) => self.parse_deref_expr(span),

            _ => Err(self.unexpected(&[Expected::Expr])),
        }
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> PResult<Expr> {
        loop {
            let span = expr.span;
            match self.peek_kind() {
                tok!(OpenParen) => {
                    self.bump();
                    let mut args = Vec::new();
                    while !self.check(tok!(CloseParen)) && !self.at_eof() {
                        args.push(self.parse_expr()?);
                        if !self.eat(tok!(,)) {
                            break;
                        }
                    }
                    let end = self.expect(tok!(CloseParen))?.span;
                    expr = self.mk_expr(ExprKind::Call(Box::new(expr), args), span.extend(end));
                }

                tok!(OpenBracket) => {
                    self.bump();
                    let index = self.parse_expr()?;
                    let end = self.expect(tok!(CloseBracket))?.span;
                    expr = self.mk_expr(
                        ExprKind::Index(Box::new(expr), Box::new(index)),
                        span.extend(end),
                    );
                }

                tok!(.) => {
                    self.bump();
                    expr = self.parse_field_or_tuple_field(expr, span)?;
                }

                tok!(?) => {
                    let end = self.bump().span;
                    expr = self.mk_expr(ExprKind::Try(Box::new(expr)), span.extend(end));
                }

                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_let_expr(&mut self, base_span: Span) -> PResult<Expr> {
        let start = self.expect(tok!(let))?.span;

        let pat: Pat = self.parse()?;

        self.expect(tok!(=))?;

        let expr = self.parse_expr()?;

        let span = start.extend(expr.span);
        Ok(self.mk_expr(ExprKind::Let(Box::new(pat), Box::new(expr)), span))
    }

    fn parse_field_or_tuple_field(&mut self, base: Expr, base_span: Span) -> PResult<Expr> {
        match self.peek_kind() {
            TokenKind::Ident => {
                let name: Ident = self.parse()?;
                let end = name.span;
                Ok(self.mk_expr(ExprKind::Field(Box::new(base), name), base_span.extend(end)))
            }
            TokenKind::Literal {
                kind: LiteralKind::Integer { .. },
                suffix: None,
            } => {
                let lit: Lit = self.parse()?;
                let ident = Ident {
                    text: lit.value,
                    span: lit.span,
                };
                let end = lit.span;
                Ok(self.mk_expr(
                    ExprKind::Field(Box::new(base), ident),
                    base_span.extend(end),
                ))
            }
            TokenKind::Literal {
                kind: LiteralKind::Integer { .. },
                suffix: Some(_),
            } => {
                let span = self.current_span();
                Err(self.emit(Diag::error("tuple index cannot have a suffix").with_span(span)))
            }
            _ => Err(self.unexpected(&[Expected::Ident])),
        }
    }

    fn parse_paren_or_tuple(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(OpenParen))?.span;

        // 括号内恢复 struct literal——整个解析过程都在允许语境
        self.with_struct_literal_allowed(|ctx| {
            // `()`
            if ctx.check(tok!(CloseParen)) {
                let end = ctx.bump().span;
                return Ok(ctx.mk_expr(ExprKind::Tuple(vec![]), start.extend(end)));
            }

            let first = ctx.parse_expr_with_precedence(Precedence::Jump)?;

            // `(e)`
            if ctx.check(tok!(CloseParen)) {
                let end = ctx.bump().span;
                return Ok(ctx.mk_expr(ExprKind::Grouped(Box::new(first)), start.extend(end)));
            }

            // `(e, ...)`
            let mut elements = vec![first];
            while ctx.eat(tok!(,)) {
                if ctx.check(tok!(CloseParen)) {
                    break;
                }
                elements.push(ctx.parse_expr_with_precedence(Precedence::Jump)?);
            }
            let end = ctx.expect(tok!(CloseParen))?.span;
            Ok(ctx.mk_expr(ExprKind::Tuple(elements), start.extend(end)))
        })
    }

    fn parse_array_literal(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(OpenBracket))?.span;

        // `[]`
        if self.check(tok!(CloseBracket)) {
            let end = self.bump().span;
            return Ok(self.mk_expr(ExprKind::Array(vec![]), start.extend(end)));
        }

        let first: Expr = self.parse()?;

        // `[value; count]`
        if self.eat(tok!(;)) {
            let count: Expr = self.parse()?;
            let end = self.expect(tok!(CloseBracket))?.span;
            return Ok(self.mk_expr(
                ExprKind::ArrayRepeat(Box::new(first), Box::new(count)),
                start.extend(end),
            ));
        }

        // `[e1, e2, ...]`
        let mut elements = vec![first];
        while self.eat(tok!(,)) {
            if self.check(tok!(CloseBracket)) {
                break;
            }
            elements.push(self.parse_expr()?);
        }
        let end = self.expect(tok!(CloseBracket))?.span;
        Ok(self.mk_expr(ExprKind::Array(elements), start.extend(end)))
    }

    fn parse_block_expr(&mut self) -> PResult<Expr> {
        let block: Block = self.parse()?;
        let span = block.span;
        Ok(self.mk_expr(ExprKind::Block(Box::new(block)), span))
    }

    fn parse_if_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(if))?.span;
        let cond =
            self.with_let_expr_allowed(|ctx| ctx.with_no_struct_literal(|ctx| ctx.parse_expr()))?;
        let then: Block = self.parse()?;

        let else_ = if self.eat(tok!(else)) {
            if self.check(tok!(if)) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let b: Block = self.parse()?;
                let span = b.span;
                Some(Box::new(self.mk_expr(ExprKind::Block(Box::new(b)), span)))
            }
        } else {
            None
        };

        let span = match &else_ {
            Some(e) => start.extend(e.span),
            None => start.extend(then.span),
        };

        Ok(self.mk_expr(ExprKind::If(Box::new(cond), then, else_), span))
    }

    fn parse_loop_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(loop))?.span;
        let body: Block = self.parse()?;
        let span = body.span;
        Ok(self.mk_expr(ExprKind::Loop(Box::new(body)), start.extend(span)))
    }

    fn parse_while_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(while))?.span;
        let cond =
            self.with_let_expr_allowed(|ctx| ctx.with_no_struct_literal(|ctx| ctx.parse_expr()))?;
        let body: Block = self.parse()?;
        let span = start.extend(body.span);
        Ok(self.mk_expr(ExprKind::While(Box::new(cond), Box::new(body)), span))
    }

    fn parse_for_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(for))?.span;
        let pat: Pat = self.parse()?;
        self.expect(tok!(in))?;
        let iter = self.with_no_struct_literal(|ctx| ctx.parse_expr())?;
        let body: Block = self.parse()?;
        let span = start.extend(body.span);
        Ok(self.mk_expr(
            ExprKind::For {
                variable: Box::new(pat),
                iter: Box::new(iter),
                body: Box::new(body),
            },
            span,
        ))
    }

    fn parse_match_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(match))?.span;
        let scrutinee = self.with_no_struct_literal(|ctx| ctx.parse_expr())?;
        self.expect(tok!(OpenBrace))?;

        let mut arms = Vec::new();
        while !self.check(tok!(CloseBrace)) && !self.at_eof() {
            let pat: Pat = self.parse()?;

            let guard = if self.eat(tok!(if)) {
                Some(Box::new(
                    self.with_no_struct_literal(|ctx| ctx.parse_expr())?,
                ))
            } else {
                None
            };

            self.expect(tok!(=>))?;
            let body = self.parse_expr()?;

            let span = pat.span.extend(body.span);

            let is_block = matches!(body.kind, ExprKind::Block(_));

            arms.push(Arm {
                pat,
                guard,
                body: Box::new(body),
                span,
                node_id: self.node_id(),
            });

            if is_block {
                self.eat(tok!(,));
            } else if self.check(tok!(CloseBrace)) {
            } else if !self.eat(tok!(,)) {
                return Err(self.unexpected(&[Expected::Exact(tok!(,))]));
            }
        }

        let end = self.expect(tok!(CloseBrace))?.span;
        Ok(self.mk_expr(
            ExprKind::Match(Box::new(scrutinee), arms),
            start.extend(end),
        ))
    }

    fn parse_return_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(return))?.span;
        if self.check(tok!(;)) || self.check(tok!(CloseBrace)) {
            return Ok(self.mk_expr(ExprKind::Return(None), start));
        }
        let value = self.parse_expr()?;
        let span = start.extend(value.span);
        Ok(self.mk_expr(ExprKind::Return(Some(Box::new(value))), span))
    }

    fn parse_break_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(tok!(break))?.span;
        if self.check(tok!(;)) || self.check(tok!(CloseBrace)) {
            return Ok(self.mk_expr(ExprKind::Break(None), start));
        }
        let value = self.parse_expr()?;
        let span = start.extend(value.span);
        Ok(self.mk_expr(ExprKind::Break(Some(Box::new(value))), span))
    }

    fn parse_unary(&mut self, op: UnOp, span: Span) -> PResult<Expr> {
        self.bump();
        let operand = self.parse_expr_with_precedence(Precedence::Prefix)?;
        let span = span.extend(operand.span);
        Ok(self.mk_expr(ExprKind::Unary(op, Box::new(operand)), span))
    }

    fn parse_ref_expr(&mut self, mutability: Mutability, span: Span) -> PResult<Expr> {
        self.bump();
        let operand = self.parse_expr_with_precedence(Precedence::Prefix)?;
        let span = span.extend(operand.span);
        Ok(self.mk_expr(ExprKind::AddressOf(mutability, Box::new(operand)), span))
    }

    fn parse_deref_expr(&mut self, span: Span) -> PResult<Expr> {
        self.bump();
        let operand = self.parse_expr_with_precedence(Precedence::Prefix)?;
        let span = span.extend(operand.span);
        Ok(self.mk_expr(ExprKind::Deref(Box::new(operand)), span))
    }
}

impl Parse for Block {
    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        todo!()
    }
}

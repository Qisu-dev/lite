use litec_ast::{
    TokenKind,
    ast::{
        ArrayLen, Expr, ExprKind, GenericArg, GenericArgKind, GenericArgs, Mutability, Path, Ty,
        TyKind,
    },
    tok,
    token::Lit,
};
use litec_error::PResult;

use crate::parser::{Expected, ParseCtx, trait_::Parse};

impl Parse for GenericArgs {
    fn parse(ctx: &mut super::ParseCtx) -> litec_error::PResult<Self> {
        let start = ctx.expect(tok!(<))?;
        let mut args = Vec::new();

        ctx.split_shr_to_gt();
        while !(ctx.peek_kind() == tok!(>)) {
            args.push(ctx.parse::<GenericArg>()?);
            ctx.split_shr_to_gt();
            if !ctx.eat(tok!(,)) {
                break;
            }
            ctx.split_shr_to_gt();
        }

        let end = ctx.expect(tok!(>))?;
        Ok(GenericArgs {
            args,
            span: start.span.extend(end.span),
        })
    }
}

impl Parse for GenericArg {
    fn parse(ctx: &mut super::ParseCtx) -> litec_error::PResult<Self> {
        let span = ctx.current_span();
        if ctx.eat(tok!(OpenBrace)) {
            let expr: Expr = ctx.parse()?;

            let close = ctx.expect(tok!(CloseBrace))?;

            return Ok(GenericArg {
                kind: GenericArgKind::Const(Box::new(expr)),
                span: span.extend(close.span),
            });
        }
        if let Some(lit) = ctx.parse_if::<Lit>(|k| k.is_literal())? {
            let span = span.extend(lit.span);
            return Ok(GenericArg {
                kind: GenericArgKind::Const(Box::new(ctx.mk_expr(ExprKind::Literal(lit), span))),
                span,
            });
        }
        let ty: Ty = ctx.parse()?;

        let span = span.extend(ty.span);
        Ok(GenericArg {
            kind: GenericArgKind::Type(Box::new(ty)),
            span,
        })
    }
}

impl Parse for Ty {
    fn parse(ctx: &mut ParseCtx) -> PResult<Self> {
        ctx.parse_ty()
    }
}

impl ParseCtx<'_> {
    pub(crate) fn parse_ty(&mut self) -> PResult<Ty> {
        match self.peek_kind() {
            tok!(OpenParen) => self.parse_ty_tuple(),

            // 指针
            tok!(*) => self.parse_ty_ptr(),

            tok!(dyn) => self.parse_ty_dyn(),
            // 函数指针
            tok!(fn) => self.parse_ty_fn(),

            // Never
            tok!(!) => {
                let span = self.bump().span;
                Ok(self.mk_ty(TyKind::Never, span))
            }
            // Infer
            tok!(_) => {
                let span = self.bump().span;
                Ok(self.mk_ty(TyKind::Infer, span))
            }

            // 数组 / 切片 —— 同入口，看 `;` 区分
            tok!(OpenBracket) => self.parse_ty_array_or_slice(),

            // 路径——Ident / Self / crate / super / ::
            TokenKind::Ident
            | TokenKind::SelfLower
            | TokenKind::SelfUpper
            | TokenKind::Crate
            | TokenKind::Super
            | TokenKind::PathAccess => {
                let path: Path = self.parse()?;
                let span = path.span;
                Ok(self.mk_ty(TyKind::Path(path), span))
            }

            _ => Err(self.unexpected(&[
                Expected::Ty
            ])),
        }
    }

    /// `()` / `(T)` / `(T,)` / `(T, U, ...)`
    fn parse_ty_tuple(&mut self) -> PResult<Ty> {
        let start = self.expect(tok!(OpenParen))?.span;

        // `()`
        if self.check(tok!(CloseParen)) {
            let end = self.bump().span;
            return Ok(self.mk_ty(TyKind::Unit, start.extend(end)));
        }

        let first: Ty = self.parse()?;

        // `(T)` —— parenthesized，折叠成内层类型
        if self.check(tok!(CloseParen)) {
            self.bump();
            return Ok(first);
        }

        // `(T,)` 或 `(T, U, ...)`
        let mut elems = vec![first];
        while self.eat(tok!(,)) {
            if self.check(tok!(CloseParen)) {
                break;
            }
            elems.push(self.parse()?);
        }
        let end = self.expect(tok!(CloseParen))?.span;
        Ok(self.mk_ty(TyKind::Tuple(elems), start.extend(end)))
    }

    /// `*T` / `*mut T`
    fn parse_ty_ptr(&mut self) -> PResult<Ty> {
        let start = self.expect(tok!(*))?.span;
        let mutability = if self.eat(tok!(mut)) {
            Mutability::Mut
        } else {
            Mutability::Immut
        };
        let inner: Ty = self.parse()?;
        let span = start.extend(inner.span);
        Ok(self.mk_ty(TyKind::Ptr(mutability, Box::new(inner)), span))
    }

    /// `fn(A, B) -> C`
    fn parse_ty_fn(&mut self) -> PResult<Ty> {
        let start = self.expect(tok!(fn))?.span;
        self.expect(tok!(OpenParen))?;

        let mut params = Vec::new();
        while !self.check(tok!(CloseParen)) && !self.at_eof() {
            params.push(self.parse()?);
            if !self.eat(tok!(,)) {
                break;
            }
        }
        let span = self.expect(tok!(CloseParen))?.span;

        let ret = if self.eat(tok!(->)) {
            Some(Box::new(self.parse_ty()?))
        } else {
            None
        };

        let span = match &ret {
            Some(r) => start.extend(r.span),
            None => start.extend(span),
        };
        Ok(self.mk_ty(TyKind::FnPtr(params, ret), span))
    }

    /// `[T]` 或 `[T; N]`
    fn parse_ty_array_or_slice(&mut self) -> PResult<Ty> {
        let start = self.expect(tok!(OpenBracket))?.span;
        let elem: Ty = self.parse()?;

        // `[T; N]`
        if self.eat(tok!(;)) {
            let len = self.parse_array_len()?;
            let end = self.expect(tok!(CloseBracket))?.span;
            return Ok(self.mk_ty(
                TyKind::Array {
                    elem: Box::new(elem),
                    len,
                },
                start.extend(end),
            ));
        }

        // `[T]`
        let end = self.expect(tok!(CloseBracket))?.span;
        Ok(self.mk_ty(TyKind::Slice(Box::new(elem)), start.extend(end)))
    }

    fn parse_array_len(&mut self) -> PResult<ArrayLen> {
        if self.eat(tok!(_)) {
            return Ok(ArrayLen::Infer);
        }
        let expr: Expr = self.parse()?;
        Ok(ArrayLen::Expr(Box::new(expr)))
    }

    fn parse_ty_dyn(&mut self) -> PResult<Ty> {
        let start = self.expect(tok!(dyn))?.span;

        let first: Path = self.parse()?;

        let mut paths = vec![first];
        while self.eat(tok!(+)) {
            paths.push(self.parse()?);
        }
        let span = start.extend(paths.last().unwrap().span);
        Ok(self.mk_ty(TyKind::Dyn(paths), span))
    }
}

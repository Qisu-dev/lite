use crate::parser::{
    ParseCtx,
    recovery::{Recovery, SYNC_STMT},
    trait_::Parse,
};
use litec_ast::{
    ast::{Expr, ExprKind, Item, Pat, Stmt, StmtKind, Ty},
    tok,
};
use litec_error::PResult;
use litec_span::Span;

impl Parse for Stmt {
    fn recovery() -> Recovery {
        Recovery::SkipTo(SYNC_STMT)
    }

    fn parse(ctx: &mut ParseCtx) -> PResult<Self> {
        let start = ctx.current_span();

        // `let`
        if ctx.check(tok!(let)) {
            return parse_let(ctx, start);
        }

        // `defer`
        if ctx.check(tok!(defer)) {
            return parse_defer(ctx, start);
        }

        // 嵌套 item
        if ctx.peek_kind().is_item_start() {
            let item: Item = ctx.parse()?;
            let span = item.span;
            return Ok(Stmt {
                kind: StmtKind::Item(Box::new(item)),
                span,
                node_id: ctx.node_id(),
            });
        }

        // 表达式语句
        let expr: Expr = ctx.parse()?;
        let expr_span = expr.span;

        if ctx.check(tok!(;)) {
            let span = expr_span.extend(ctx.bump().span);
            Ok(Stmt {
                kind: StmtKind::Semi(Box::new(expr)),
                span,
                node_id: ctx.node_id(),
            })
        } else {
            Ok(Stmt {
                kind: StmtKind::Expr(Box::new(expr)),
                span: expr_span,
                node_id: ctx.node_id(),
            })
        }
    }
}

fn parse_let(ctx: &mut ParseCtx, start: Span) -> PResult<Stmt> {
    ctx.expect(tok!(let))?;
    let pat: Pat = ctx.parse()?;

    let ty = if ctx.eat(tok!(:)) {
        Some(Box::new(ctx.parse::<Ty>()?))
    } else {
        None
    };

    let init = if ctx.eat(tok!(=)) {
        Some(Box::new(ctx.parse::<Expr>()?))
    } else {
        None
    };

    let semi = ctx.expect(tok!(;))?;
    let span = start.extend(semi.span);

    Ok(Stmt {
        kind: StmtKind::Let(Box::new(pat), ty, init),
        span,
        node_id: ctx.node_id(),
    })
}

fn parse_defer(ctx: &mut ParseCtx, _start: Span) -> PResult<Stmt> {
    ctx.expect(tok!(defer))?;
    let expr: Expr = ctx.parse()?;
    let is_block = matches!(expr.kind, ExprKind::Block(_));
    let expr_span = expr.span;

    let span = if is_block && !ctx.check(tok!(;)) {
        expr_span
    } else {
        let semi = ctx.expect(tok!(;))?;
        expr_span.extend(semi.span)
    };

    Ok(Stmt {
        kind: StmtKind::Defer(Box::new(expr)),
        span,
        node_id: ctx.node_id(),
    })
}

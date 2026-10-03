use crate::parser::recovery::SyncSet;
use crate::parser::trait_::Parse;
use crate::{
    lexer::Lexer,
    parser::recovery::{Recovery, RecoveryResult},
};
use litec_ast::ast::{Expr, ExprKind, NodeId, Pat, PatKind, Stmt, StmtKind, Ty, TyKind};
use litec_ast::token::Token;
use litec_ast::{TokenKind, tok};
use litec_error::{Diag, ErrorGuaranteed, PResult};
use litec_session::Session;
use litec_span::{FileId, Span, symbols};

mod expr;
mod item;
mod pat;
mod recovery;
mod stmt;
#[cfg(test)]
mod tests;
mod trait_;
mod ty;

pub(crate) struct ParseCtx<'a, 'src> {
    lexer: Lexer<'a, 'src>,
    buffer: Vec<Token>,
    cursor: usize,
    session: &'a Session,
    next_node_id: u32,
    ctx_stack: Vec<ParseContext>,
    /// 已收集但还没附着的 doc comment
    pending_outro_docs: Vec<Token>,
    no_struct_literal: bool,
    allow_let_expr: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ParseContext {
    Type,
    Expr,
    Pattern,
    Item,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mark {
    cursor: usize,
    diags_len: usize,
}

impl<'a, 'src> ParseCtx<'a, 'src> {
    pub(crate) fn new(session: &'a Session, src: &'src str, file_id: FileId) -> Self {
        Self {
            lexer: Lexer::new(session, src, file_id),
            buffer: vec![],
            cursor: 0,
            session,
            next_node_id: 0,
            ctx_stack: vec![],
            pending_outro_docs: vec![],
            no_struct_literal: false,
            allow_let_expr: false,
        }
    }

    /// 保证 buffer 里至少有 cursor + k + 1 个 token
    /// lexer 每次前进不会卡住，所以这个循环一定停
    fn fill(&mut self, k: usize) {
        use TokenKind::*;
        let need = self.cursor + k + 1;
        while self.buffer.len() < need {
            match self.lexer.advance() {
                Ok(tok) => match tok.kind {
                    LineComment | BlockComment | Whitespace => continue,
                    DocComment | DocLineComment | DocBlockComment => {
                        self.pending_outro_docs.push(tok);
                    }
                    _ => {
                        self.buffer.push(tok);
                    }
                },
                Err(_) => continue,
            }
        }
    }

    /// 看第 k 个 token，但不消费
    /// k 是 0 就是当前这个
    pub(crate) fn peek(&mut self, k: usize) -> Token {
        self.fill(k);
        self.buffer[self.cursor + k]
    }

    /// 当前 token 是什么 kind
    pub(crate) fn peek_kind(&mut self) -> TokenKind {
        self.peek(0).kind
    }

    /// 当前是不是指定的 kind
    pub(crate) fn check(&mut self, kind: TokenKind) -> bool {
        self.peek_kind() == kind
    }

    /// 是不是读完了
    pub(crate) fn at_eof(&mut self) -> bool {
        self.peek_kind() == TokenKind::Eof
    }

    /// 当前 token 的位置
    pub(crate) fn current_span(&mut self) -> Span {
        self.peek(0).span
    }

    /// 吃掉一个 token，让游标往前一格
    /// 一定成功，因为 fill 已经保证 buffer 里有东西
    pub(crate) fn bump(&mut self) -> Token {
        self.fill(0);
        let tok = self.buffer[self.cursor];
        self.cursor += 1;
        tok
    }

    /// 如果当前是指定的 kind，就吃掉它
    pub(crate) fn eat(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// 一定要是指定的 kind，不然就报错
    /// 这是唯一会返回错误的访问器，因为它代表真正的语法错误
    pub(crate) fn expect(&mut self, expected: TokenKind) -> PResult<Token> {
        if self.peek_kind() == expected {
            Ok(self.bump())
        } else {
            Err(self.unexpected(&[expected.into()]))
        }
    }

    pub(crate) fn expect_literal(&mut self) -> PResult<Token> {
        if self.peek_kind().is_literal() {
            Ok(self.bump())
        } else {
            Err(self.unexpected(&[Expected::Literal]))
        }
    }

    pub fn mark(&self) -> Mark {
        Mark {
            cursor: self.cursor,
            diags_len: self.session.dcx().len(),
        }
    }

    pub fn restore(&mut self, mark: Mark) {
        debug_assert!(mark.cursor <= self.cursor);
        self.cursor = mark.cursor;
        self.session.dcx().truncate(mark.diags_len);
    }

    /// 报一个诊断
    pub(crate) fn emit(&self, diag: Diag) -> ErrorGuaranteed {
        self.session.dcx().emit_err(diag)
    }

    /// 现在已经报了多少个诊断
    pub(crate) fn diags_count(&self) -> usize {
        self.session.dcx().len()
    }

    /// 生成 expected X found Y 这种错误
    pub(crate) fn unexpected(&mut self, expected: &[Expected]) -> ErrorGuaranteed {
        let span = self.current_span();
        let found = self.peek_kind();
        let expected_str = join_expected(expected);
        let msg = format!("expected {}, found {}", expected_str, found.describe());
        self.emit(Diag::error(msg).with_span(span))
    }

    /// 跳到同步点——停在它前面，不消费
    pub(crate) fn skip_to(&mut self, sync: SyncSet) -> RecoveryResult {
        while !self.at_eof() {
            if sync.matches(self.peek_kind()) {
                return RecoveryResult::Ok;
            }
            self.bump();
        }
        RecoveryResult::Failed
    }

    /// 解析 `end` 之前的所有 `T`，可选 `sep` 分隔。
    ///
    /// 每个元素失败时：
    /// - 若 `T::error_node()` 返回 `Some` → 产错误节点 + 跑 `T::recovery()` → 继续
    /// - 若返回 `None` → 向上传播 `Err`
    ///
    /// 只有列表边界——不递归到 `T::parse` 内部。
    pub(crate) fn parse_list<T: Parse>(
        &mut self,
        end: TokenKind,
        sep: Option<TokenKind>,
    ) -> PResult<Vec<T>> {
        let mut items = Vec::new();

        while !self.check(end) && !self.at_eof() {
            let before = self.cursor;

            match T::parse(self) {
                Ok(v) => items.push(v),
                Err(_) => {
                    // 错误已 emit —— 直接跑恢复
                    if run_recovery(T::recovery(), self) == RecoveryResult::Failed {
                        break;
                    }
                }
            }

            if let Some(s) = sep {
                if !self.eat(s) && !self.check(end) {
                    let span = self.current_span();
                    let kind = self.peek_kind();
                    self.emit(
                        Diag::error(format!(
                            "expected `{}`, found `{}`",
                            s.describe(),
                            kind.describe(),
                        ))
                        .with_span(span)
                        .with_label(span, format!("expected `{}` here", s.describe())),
                    );
                    if run_recovery(T::recovery(), self) == RecoveryResult::Failed {
                        break;
                    }
                }
            }

            // 防死循环 —— 确保前进
            if self.cursor == before {
                self.bump();
            }
        }

        let _ = self.expect(end);
        Ok(items)
    }

    /// 尝试解析 `T`，失败则回滚——不报错。
    /// 用于"可有可无"的语法，比如 `where` 子句、返回类型。
    pub(crate) fn parse_optional<T: Parse>(&mut self) -> Option<T> {
        let mark = self.mark();
        match T::parse(self) {
            Ok(v) => Some(v),
            Err(_) => {
                self.restore(mark);
                None
            }
        }
    }

    pub(crate) fn node_id(&mut self) -> NodeId {
        let id = NodeId::from_raw(self.next_node_id);
        self.next_node_id += 1;
        id
    }

    pub(crate) fn parse<T: Parse>(&mut self) -> PResult<T> {
        T::parse(self)
    }

    fn with_context<T>(
        &mut self,
        ctx: ParseContext,
        f: impl FnOnce(&mut Self) -> PResult<T>,
    ) -> PResult<T> {
        self.ctx_stack.push(ctx);
        let result = f(self);
        self.ctx_stack.pop();
        result
    }

    fn current_context(&self) -> ParseContext {
        self.ctx_stack.last().copied().unwrap_or(ParseContext::Type)
    }

    //// 把当前位置的 `>>` 拆成两个 `>`。
    /// 只在泛型解析里调用——其他路径看到 `>>` 就是右移。
    fn split_shr_to_gt(&mut self) {
        if self.peek_kind() != tok!(>>) {
            return;
        }
        let tok = self.buffer[self.cursor].clone();
        self.buffer[self.cursor] = Token {
            kind: TokenKind::Gt,
            span: Span {
                end: tok.span.start + 1,
                ..tok.span
            },
            text: symbols::Gt,
        };
        self.buffer.insert(
            self.cursor + 1,
            Token {
                kind: TokenKind::Gt,
                span: Span {
                    start: tok.span.start + 1,
                    ..tok.span
                },
                text: symbols::Gt,
            },
        );
    }

    /// 把当前位置的 `||` 拆成两个 `|`。
    /// 只在闭包参数列表解析里调用——其他路径看到 `||` 就是逻辑或。
    fn split_or_to_bit_or(&mut self) {
        if self.peek_kind() != tok!(||) {
            return;
        }
        let tok = self.buffer[self.cursor].clone();
        self.buffer[self.cursor] = Token {
            kind: TokenKind::BitOr,
            span: Span {
                end: tok.span.start + 1,
                ..tok.span
            },
            text: symbols::BitOr,
        };
        self.buffer.insert(
            self.cursor + 1,
            Token {
                kind: TokenKind::BitOr,
                span: Span {
                    start: tok.span.start + 1,
                    ..tok.span
                },
                text: symbols::BitOr,
            },
        );
    }

    /// 取走积累的 doc comment——清空 pending_docs。
    ///
    /// 每个 item 的 `parse` 入口无条件调用（即使当前没有 doc）。
    /// 先 take 再解析——失败早退也不残留。
    pub(crate) fn take_docs(&mut self) -> Vec<Token> {
        // 保证 cursor 前的 doc 已被收集
        // （cursor 之前的 token 一定已经 lex 过，所以这一步通常不跑）
        self.fill(0);
        std::mem::take(&mut self.pending_outro_docs)
    }

    /// 检查有没有残留的 doc comment。
    ///
    /// 文件末尾调用——有残留说明这些 doc 没附着到任何 item。
    /// 清空并报错——不会留下脏状态。
    pub(crate) fn check_no_pending_docs(&mut self) -> PResult<()> {
        // 确保 EOF 前的 doc 已被收集
        self.fill(0);

        if self.pending_outro_docs.is_empty() {
            return Ok(());
        }

        let first = &self.pending_outro_docs[0];
        let span = first.span;
        let count = self.pending_outro_docs.len();

        // 无条件清空——不留脏状态
        self.pending_outro_docs.clear();

        let msg = if count == 1 {
            "unattached doc comment".to_string()
        } else {
            format!("{} unattached doc comments", count)
        };

        Err(self.emit(
            Diag::error(msg)
                .with_span(span)
                .with_label(span, "this doc comment has nothing to attach to"),
        ))
    }

    pub(crate) fn mk_expr(&mut self, kind: ExprKind, span: Span) -> Expr {
        Expr {
            node_id: self.node_id(),
            kind,
            span,
        }
    }

    fn parse_if<T: Parse>(&mut self, f: impl FnOnce(TokenKind) -> bool) -> PResult<Option<T>> {
        if f(self.peek_kind()) {
            Ok(Some(self.parse()?))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn mk_ty(&mut self, kind: TyKind, span: Span) -> Ty {
        Ty {
            node_id: self.node_id(),
            kind,
            span,
        }
    }

    pub(crate) fn mk_stmt(&mut self, kind: StmtKind, span: Span) -> Stmt {
        Stmt {
            node_id: self.node_id(),
            kind,
            span,
        }
    }

    pub(crate) fn mk_pat(&mut self, kind: PatKind, span: Span) -> Pat {
        Pat {
            node_id: self.node_id(),
            kind,
            span,
        }
    }

    fn with_no_struct_literal<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        let prev = self.no_struct_literal;
        self.no_struct_literal = true;
        let result = f(self);
        self.no_struct_literal = prev;
        result
    }

    /// 在允许 struct literal 的语境下运行闭包。
    /// 进出时保存/恢复——嵌套正确，`?` 早退也恢复。
    pub(crate) fn with_struct_literal_allowed<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> PResult<T>,
    ) -> PResult<T> {
        let prev = self.no_struct_literal;
        self.no_struct_literal = false;
        let result = f(self);
        self.no_struct_literal = prev;
        result
    }

    pub(crate) fn with_let_expr_allowed<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> PResult<T>,
    ) -> PResult<T> {
        let prev = self.allow_let_expr;
        self.allow_let_expr = true;
        let result = f(self);
        self.allow_let_expr = prev;
        result
    }

    pub(crate) fn with_let_expr_disallowed<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> PResult<T>,
    ) -> PResult<T> {
        let prev = self.allow_let_expr;
        self.allow_let_expr = false;
        let result = f(self);
        self.allow_let_expr = prev;
        result
    }

    pub(crate) fn prev_span(&self) -> Span {
        self.buffer[self.cursor - 1].span
    }
}

pub(crate) fn run_recovery(recovery: Recovery, ctx: &mut ParseCtx<'_, '_>) -> RecoveryResult {
    match recovery {
        Recovery::Fatal => RecoveryResult::Failed,

        Recovery::Emit => RecoveryResult::Ok,

        Recovery::Delete => {
            if !ctx.at_eof() {
                ctx.bump();
            }
            RecoveryResult::Ok
        }

        Recovery::SkipTo(sync) => ctx.skip_to(sync),

        Recovery::Seq(rs) => {
            for r in rs {
                if run_recovery(*r, ctx) == RecoveryResult::Failed {
                    return RecoveryResult::Failed;
                }
            }
            RecoveryResult::Ok
        }

        Recovery::First(rs) => {
            for r in rs {
                let mark = ctx.mark();
                if run_recovery(*r, ctx) == RecoveryResult::Ok {
                    return RecoveryResult::Ok;
                }
                // 失败——回滚——试下一个
                ctx.restore(mark);
            }
            RecoveryResult::Failed
        }

        Recovery::When(cond, inner) => {
            if cond.eval(ctx) {
                run_recovery(*inner, ctx)
            } else {
                RecoveryResult::Ok
            }
        }

        Recovery::Bounded { max, inner } => {
            if ctx.diags_count() >= max {
                RecoveryResult::Failed
            } else {
                run_recovery(*inner, ctx)
            }
        }

        Recovery::Custom { f, .. } => f(ctx),
    }
}

/// parser 期望的 token——支持精确匹配和类别匹配。
///
/// `Exact` 用于「期望某个具体 TokenKind」（如 `expect(TokenKind::Semi)`）。
/// `Literal` 用于「期望任意字面量」——`TokenKind::Literal` 带数据，
/// 没法用一个具体值表示「任意字面量」，所以走类别匹配。
pub(crate) enum Expected {
    Exact(TokenKind),
    Literal,
    Expr,
    Ident,
    Path,
    Ty,
    Pattern,
}

impl Expected {
    fn describe(&self) -> String {
        match self {
            Expected::Exact(kind) => kind.describe().to_string(),
            Expected::Literal => "literal".into(),
            Expected::Expr => "expression".into(),
            Expected::Ident => "identifier".into(),
            Expected::Path => "path".into(),
            Expected::Ty => "type".into(),
            Expected::Pattern => "pattern".into(),
        }
    }
}

fn join_expected(expected: &[Expected]) -> String {
    match expected {
        [] => "something".into(),
        [a] => a.describe(),
        [a, b] => format!("{} or {}", a.describe(), b.describe()),
        [init @ .., last] => {
            let head = init
                .iter()
                .map(|e| e.describe())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}, or {}", head, last.describe())
        }
    }
}

impl From<TokenKind> for Expected {
    fn from(kind: TokenKind) -> Self {
        Expected::Exact(kind)
    }
}

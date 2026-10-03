use litec_ast::{TokenKind, tok};

use crate::parser::ParseCtx;

#[derive(Clone, Copy)]
pub(crate) struct SyncSet(&'static [TokenKind]);

impl SyncSet {
    pub(crate) fn matches(&self, kind: TokenKind) -> bool {
        self.0.contains(&kind)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Recovery {
    /// 不恢复，错误直接向上传播
    Fatal,
    /// 位置不动，只产出错误节点
    Emit,
    /// 吞掉一个 token
    Delete,
    /// 跳到同步点之前停住
    SkipTo(SyncSet),
    /// 顺序组合，子恢复依次执行，任一失败则整体失败
    Seq(&'static [Recovery]),
    /// 择一组合，按顺序尝试，第一个成功即返回，失败则回滚试下一个
    First(&'static [Recovery]),
    /// 条件执行，条件为真才跑内层恢复
    When(Cond, &'static Recovery),
    /// 错误预算，报错数超过 max 就放弃
    Bounded {
        max: usize,
        inner: &'static Recovery,
    },
    /// 逃生舱，任意自定义恢复逻辑
    Custom { name: &'static str, f: RecoverFn },
}

#[derive(Clone, Copy)]
pub(crate) enum Cond {
    /// 已报诊断数小于 n
    ErrorCountLt(usize),
    /// 当前 token 属于同步集
    At(SyncSet),
    /// 当前 token 不属于同步集
    NotAt(SyncSet),
    /// 全部为真
    And(&'static [Cond]),
    /// 任一为真
    Or(&'static [Cond]),
    /// 取反
    Not(&'static Cond),
}

impl Cond {
    pub(crate) fn eval(&self, ctx: &mut ParseCtx<'_, '_>) -> bool {
        match self {
            Cond::ErrorCountLt(n) => ctx.diags_count() < *n,
            Cond::At(sync) => sync.matches(ctx.peek_kind()),
            Cond::NotAt(sync) => !sync.matches(ctx.peek_kind()),
            Cond::And(cs) => cs.iter().all(|c| c.eval(ctx)),
            Cond::Or(cs) => cs.iter().any(|c| c.eval(ctx)),
            Cond::Not(c) => !c.eval(ctx),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryResult {
    Ok,     // 已恢复
    Failed, // 放弃
}

pub(crate) type RecoverFn = fn(&mut ParseCtx<'_, '_>) -> RecoveryResult;

/// 顶层 item 列表
pub(crate) const SYNC_ITEM: SyncSet = SyncSet(&[
    tok!(fn),
    tok!(struct),
    tok!(enum),
    tok!(union),
    tok!(trait),
    tok!(impl),
    tok!(mod),
    tok!(use),
    tok!(const),
    tok!(static),
    tok!(extern),
    tok!(pub),
    tok!(priv),
]);

/// 块内语句列表
pub(crate) const SYNC_STMT: SyncSet = SyncSet(&[
    // 语句开始
    tok!(let),
    tok!(if),
    tok!(while),
    tok!(for),
    tok!(loop),
    tok!(match),
    tok!(return),
    tok!(break),
    tok!(continue),
    // 语句结束 / 块结束
    tok!(;),
    tok!(CloseBrace),
]);

/// 类型位置
pub(crate) const SYNC_TY: SyncSet = SyncSet(&[
    tok!(,),
    tok!(;),
    tok!(CloseParen),
    tok!(CloseBracket),
    tok!(CloseBrace),
    tok!(=),
    tok!(->),
    tok!(=>),
    tok!(where),
]);

/// 表达式列表
pub(crate) const SYNC_EXPR: SyncSet = SyncSet(&[
    tok!(,),
    tok!(;),
    tok!(CloseParen),
    tok!(CloseBracket),
    tok!(CloseBrace),
]);

/// match 分支
pub(crate) const SYNC_MATCH_ARM: SyncSet = SyncSet(&[tok!(=>), tok!(,), tok!(CloseBrace)]);

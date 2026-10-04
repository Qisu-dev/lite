use litec_error::PResult;
use litec_span::Span;

use super::ParseCtx;
use super::recovery::Recovery;

pub(crate) trait Parse: Sized {
    /// 该类型解析失败时的默认恢复策略
    fn recovery() -> Recovery {
        Recovery::Fatal
    }

    /// 失败时产的错误占位节点——`None` = 不能占位
    fn error_node(_span: Span) -> Option<Self> {
        None
    }

    fn parse(ctx: &mut ParseCtx) -> PResult<Self>;
}

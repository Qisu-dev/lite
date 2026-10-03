pub mod options;
pub mod target;

use std::sync::Arc;

use litec_error::{Diag, DiagCtxt, ErrorGuaranteed};
use litec_span::{SourceMap, with_session_globals};

pub use crate::options::{OptLevel, SessOptions};
pub use crate::target::{Arch, Os, TargetTriple};

/// 一次编译会话
/// [`Session`] **不是** TLS 里的 [`litec_span::SessionGlobals`]
/// [`Session`] 是每次编译运行独立的上下文
#[derive(Debug, Clone)]
pub struct Session {
    /// 编译选项（只读）
    opts: SessOptions,

    /// 本次运行的诊断收集器
    dcx: DiagCtxt,

    /// 源文件映射(从 [`litec_span::SessionGlobals`] 拿的一份克隆)
    source_map: Arc<SourceMap>,
}

impl Session {
    /// 创建会话要求 [`litec_span::SessionGlobals`] 已初始化
    pub fn new(source_map: Arc<SourceMap>, opts: SessOptions) -> Self {
        Session {
            opts,
            dcx: DiagCtxt::new(source_map.clone()),
            source_map,
        }
    }

    pub fn opts(&self) -> &SessOptions {
        &self.opts
    }

    pub fn source_map(&self) -> &Arc<SourceMap> {
        &self.source_map
    }

    pub fn dcx(&self) -> &DiagCtxt {
        &self.dcx
    }

    pub fn emit_err(&self, diag: Diag) -> ErrorGuaranteed {
        self.dcx.emit_err(diag)
    }

    pub fn emit_warn(&self, diag: Diag) {
        self.dcx.emit_warn(diag);
    }

    pub fn error_count(&self) -> usize {
        self.dcx.len()
    }

    pub fn take_diags(&self) -> Vec<Diag> {
        self.dcx.take_diags()
    }
}

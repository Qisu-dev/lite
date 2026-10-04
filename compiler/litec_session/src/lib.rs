pub mod options;
pub mod target;

use litec_error::{Diag, DiagCtxt, ErrorGuaranteed};
use litec_span::{FileId, SourceMap};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use crate::options::{OptLevel, SessOptions};
pub use crate::target::{Arch, Os, TargetTriple};
// 一次编译会话
///
/// [`Session`] **不是** TLS 里的 [`litec_span::SessionGlobals`]
/// [`Session`] 是每次编译运行独立的上下文。
#[derive(Debug)]
pub struct Session {
    opts: SessOptions,
    dcx: DiagCtxt,
    source_map: Arc<SourceMap>,

    /// 全局 NodeId 分配器——每次编译单调递增
    next_node_id: Cell<u32>,

    /// 正在加载的模块栈——循环检测
    /// 每次 `mod foo;` 进入时 push，退出时 pop
    loading_stack: RefCell<Vec<PathBuf>>,
}

impl Session {
    pub fn new(source_map: Arc<SourceMap>, opts: SessOptions) -> Self {
        Session {
            opts,
            dcx: DiagCtxt::new(source_map.clone()),
            source_map,
            next_node_id: Cell::new(0),
            loading_stack: RefCell::new(Vec::new()),
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

    /// 分配一个新的 NodeId 全 crate 唯一
    pub fn next_id(&self) -> u32 {
        let id = self.next_node_id.get();
        self.next_node_id.set(id + 1);
        id
    }

    pub fn is_loading(&self, path: &Path) -> bool {
        self.loading_stack.borrow().iter().any(|p| p == path)
    }

    pub fn enter_module(&self, path: PathBuf) -> LoadingGuard<'_> {
        self.loading_stack.borrow_mut().push(path);
        LoadingGuard { session: self }
    }
}

pub struct LoadingGuard<'a> {
    session: &'a Session,
}

impl Drop for LoadingGuard<'_> {
    fn drop(&mut self) {
        self.session.loading_stack.borrow_mut().pop();
    }
}

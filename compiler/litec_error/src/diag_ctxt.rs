use litec_span::SourceMap;
use std::{cell::RefCell, sync::Arc};

use crate::{Diag, DiagLevel, ErrorGuaranteed};

#[derive(Debug, Clone)]
pub struct DiagCtxt {
    source_map: Arc<SourceMap>,
    diags: RefCell<Vec<Diag>>,
}

impl DiagCtxt {
    pub fn new(source_map: Arc<SourceMap>) -> Self {
        Self {
            source_map,
            diags: RefCell::new(Vec::new()),
        }
    }

    pub fn emit_warn(&self, diag: Diag) {
        debug_assert!(
            diag.level == DiagLevel::Warning,
            "emit_warn called with non-warning diagnostic"
        );
        self.diags.borrow_mut().push(diag);
    }

    /// 发射一个错误诊断，直接返回 ErrorGuaranteed
    pub fn emit_err(&self, diag: Diag) -> ErrorGuaranteed {
        debug_assert!(
            diag.level == DiagLevel::Error,
            "emit_err called with non-error diagnostic"
        );
        self.diags.borrow_mut().push(diag);
        ErrorGuaranteed::new()
    }

    pub fn diags_count(&self) -> usize {
        self.diags.borrow().len()
    }

    pub fn take_diags(&self) -> Vec<Diag> {
        self.diags.take()
    }

    pub fn truncate(&self, idx: usize) {
        self.diags.borrow_mut().truncate(idx);
    }

    pub fn flush(&self) {
        let diags = self.diags.borrow_mut();
        for diag in diags.clone().into_iter() {
            eprintln!("{}", diag.render(&self.source_map));
        }
    }
}

impl Drop for DiagCtxt {
    fn drop(&mut self) {
        for diag in self.diags.take() {
            eprintln!("{}", diag.render(&self.source_map))
        }
    }
}

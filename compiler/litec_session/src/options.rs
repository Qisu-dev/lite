use std::path::PathBuf;

use crate::target::TargetTriple;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    None,
    Size,
    Speed,
}

/// 纯编译选项：来自 CLI / `lite.toml`
///
/// 这是**可克隆的纯数据**，不含任何运行时状态
/// 每次编译会话构造一次，之后只读
#[derive(Debug, Clone)]
pub struct SessOptions {
    /// crate名称
    pub crate_name: String,
    /// 优化级别
    pub opt_level: OptLevel,
    /// 目标平台
    pub target: TargetTriple,
    /// 系统根目录（标准库位置）
    pub sysroot: PathBuf,
    /// 是否生成调试信息
    pub debug_info: bool,
    /// 是否生成 LLVM IR
    pub emit_llvm_ir: bool,
}

impl SessOptions {
    pub fn new(crate_name: impl Into<String>, target: TargetTriple, sysroot: PathBuf) -> Self {
        Self {
            crate_name: crate_name.into(),
            opt_level: OptLevel::None,
            target,
            sysroot,
            debug_info: false,
            emit_llvm_ir: false,
        }
    }

    pub fn with_opt_level(mut self, level: OptLevel) -> Self {
        self.opt_level = level;
        self
    }

    pub fn with_debug_info(mut self, enable: bool) -> Self {
        self.debug_info = enable;
        self
    }

    pub fn with_emit_llvm_ir(mut self, enable: bool) -> Self {
        self.emit_llvm_ir = enable;
        self
    }

    pub fn opt_level(&self) -> OptLevel {
        self.opt_level
    }

    pub fn target(&self) -> &TargetTriple {
        &self.target
    }

    pub fn sysroot(&self) -> &PathBuf {
        &self.sysroot
    }

    pub fn core_path(&self) -> PathBuf {
        self.sysroot.join("core")
    }

    pub fn std_path(&self) -> PathBuf {
        self.sysroot.join("std")
    }
}

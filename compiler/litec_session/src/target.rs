//! 目标平台描述。
//!
//! 用 `Arch` + `Os` 两个枚举组合成 `TargetTriple`，

use std::fmt;
use std::str::FromStr;

/// CPU 架构。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    X86_64,
    Aarch64,
    Riscv64,
}

impl Arch {
    pub const ALL: &'static [Arch] = &[
        Arch::X86_64,
        Arch::Aarch64,
        Arch::Riscv64,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
            Arch::Riscv64 => "riscv64",
        }
    }

    /// 宿主架构。不支持的宿主返回 `None`。
    pub fn host() -> Option<Self> {
        match std::env::consts::ARCH {
            "x86_64" => Some(Arch::X86_64),
            "aarch64" => Some(Arch::Aarch64),
            "riscv64" => Some(Arch::Riscv64),
            _ => None,
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Arch {
    type Err = TargetParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "x86_64" | "amd64" => Ok(Arch::X86_64),
            "aarch64" | "arm64" => Ok(Arch::Aarch64),
            "riscv64" => Ok(Arch::Riscv64),
            _ => Err(TargetParseError::UnknownArch(s.into())),
        }
    }
}

/// 操作系统。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Linux,
    Windows,
    Macos,
}

impl Os {
    pub const ALL: &'static [Os] = &[
        Os::Linux,
        Os::Windows,
        Os::Macos,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Os::Linux => "linux",
            Os::Windows => "windows",
            Os::Macos => "macos",
        }
    }

    /// 宿主 OS。不支持的宿主返回 `None`。
    pub fn host() -> Option<Self> {
        match std::env::consts::OS {
            "linux" => Some(Os::Linux),
            "windows" => Some(Os::Windows),
            "macos" => Some(Os::Macos),
            _ => None,
        }
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Os {
    type Err = TargetParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "linux" => Ok(Os::Linux),
            "windows" | "win" => Ok(Os::Windows),
            "macos" | "darwin" | "osx" => Ok(Os::Macos),
            _ => Err(TargetParseError::UnknownOs(s.into())),
        }
    }
}

/// 目标平台：架构 + 操作系统。
///
/// 用 `Display` 输出简写形式，例如 `x86_64-linux`。
/// 用 [`TargetTriple::to_llvm_triple`] 输出 LLVM 使用的标准三元组。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetTriple {
    pub arch: Arch,
    pub os: Os,
}

impl TargetTriple {
    pub const fn new(arch: Arch, os: Os) -> Self {
        TargetTriple { arch, os }
    }

    /// 宿主平台。不支持的宿主返回 `None`。
    pub fn host() -> Option<Self> {
        Some(TargetTriple {
            arch: Arch::host()?,
            os: Os::host()?,
        })
    }

    /// LLVM 风格的标准三元组字符串。
    ///
    /// 支持的组合有具体映射；未列出的组合退化为 `arch-os`。
    pub fn to_llvm_triple(self) -> String {
        match (self.arch, self.os) {
            (Arch::X86_64,  Os::Linux)   => "x86_64-unknown-linux-gnu".into(),
            (Arch::X86_64,  Os::Windows) => "x86_64-pc-windows-msvc".into(),
            (Arch::X86_64,  Os::Macos)   => "x86_64-apple-darwin".into(),
            (Arch::Aarch64, Os::Linux)   => "aarch64-unknown-linux-gnu".into(),
            (Arch::Aarch64, Os::Windows) => "aarch64-pc-windows-msvc".into(),
            (Arch::Aarch64, Os::Macos)   => "aarch64-apple-darwin".into(),
            (Arch::Riscv64, Os::Linux)   => "riscv64-unknown-linux-gnu".into(),
            _ => format!("{}-{}", self.arch.as_str(), self.os.as_str()),
        }
    }

    /// 可执行文件后缀（含点），例如 `.exe`。
    pub fn exe_suffix(self) -> &'static str {
        match self.os {
            Os::Windows => ".exe",
            _ => "",
        }
    }

    /// 动态库后缀，例如 `.dll`。
    pub fn dylib_suffix(self) -> &'static str {
        match self.os {
            Os::Windows => ".dll",
            Os::Macos => ".dylib",
            Os::Linux => ".so",
        }
    }
}

impl fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.arch, self.os)
    }
}

impl FromStr for TargetTriple {
    type Err = TargetParseError;

    /// 支持两种格式：
    /// - `arch-os`：例如 `x86_64-linux`
    /// - `arch-vendor-os[-abi]`：例如 `x86_64-unknown-linux-gnu`，vendor 和 abi 被忽略
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();

        let (arch_str, os_str) = match parts.len() {
            2 => (parts[0], parts[1]),
            3 => (parts[0], parts[2]),
            4 => (parts[0], parts[2]),
            _ => return Err(TargetParseError::InvalidFormat(s.into())),
        };

        Ok(TargetTriple {
            arch: arch_str.parse()?,
            os: os_str.parse()?,
        })
    }
}

// ============================================================
// Error
// ============================================================

#[derive(Debug, Clone)]
pub enum TargetParseError {
    InvalidFormat(String),
    UnknownArch(String),
    UnknownOs(String),
}

impl fmt::Display for TargetParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetParseError::InvalidFormat(s) => {
                write!(f, "invalid target triple format: `{}`", s)
            }
            TargetParseError::UnknownArch(s) => {
                write!(f, "unknown architecture: `{}`", s)
            }
            TargetParseError::UnknownOs(s) => {
                write!(f, "unknown operating system: `{}`", s)
            }
        }
    }
}

impl std::error::Error for TargetParseError {}
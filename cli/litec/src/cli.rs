//! 手写参数解析——`key=value` 风格

use std::path::PathBuf;

use litec_session::{OptLevel, TargetTriple};

pub const HELP: &str = "\
litec —— Lite 编译器

用法: 
  litec [子命令] [key=value ...]

子命令: 
  build        编译
  lex          只输出 token
  ast          只输出 ast
  help         显示本帮助
  version      显示版本

参数: 
  file=<p>[,<p>...]        输入文件（必需，逗号分隔）
  output=<p>               输出路径
  target=<triple>          目标平台(默认宿主)
  opt=<0|1|2|3|s|z>        优化级别(默认 0)
  sysroot=<p>              标准库根目录
  crate_name=<name>        crate 名(默认从文件名推)
  debug_info=<bool>        生成调试信息
  emit_llvm_ir=<bool>      输出 LLVM IR
  pretty=<bool>            ast 输出用详细格式(默认紧凑)

示例: 
  litec file=main.lite
  litec lex file=main.lite
  litec build file=a.lite,b.lite target=x86_64-linux opt=2
";

#[derive(Debug)]
pub struct Cli {
    pub command: Command,
    pub files: Vec<PathBuf>,

    #[allow(unused)]
    pub output: Option<PathBuf>,
    pub target: TargetTriple,
    pub opt_level: OptLevel,
    pub sysroot: PathBuf,
    pub crate_name: Option<String>,
    pub debug_info: bool,
    pub emit_llvm_ir: bool,
    pub pretty: bool,
}

#[derive(Debug)]
pub enum Command {
    Build,
    Lex,
    Ast,
    Help,
    Version,
}

#[derive(Debug)]
pub struct CliError(pub String);

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Cli {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, CliError> {
        // 收集到 Vec——参数顺序无所谓
        let args: Vec<String> = args.into_iter().collect();

        // 子命令
        let (command, rest) = match args.first() {
            Some(s) if !s.contains('=') => {
                let cmd = match s.as_str() {
                    "build" => Command::Build,
                    "lex" => Command::Lex,
                    "ast" => Command::Ast,
                    "help" | "--help" | "-h" => Command::Help,
                    "version" | "--version" | "-V" => Command::Version,
                    _ => return Err(CliError(format!("未知子命令: `{s}`"))),
                };
                (cmd, &args[1..])
            }
            _ => (Command::Build, &args[..]),
        };

        // 子命令已确定的直接返回
        if matches!(command, Command::Help | Command::Version) {
            return Ok(Cli {
                command,
                files: Vec::new(),
                output: None,
                target: TargetTriple::host().ok_or_else(|| CliError("不支持的宿主平台".into()))?,
                opt_level: OptLevel::None,
                sysroot: PathBuf::from("/"),
                crate_name: None,
                debug_info: false,
                emit_llvm_ir: false,
                pretty: false,
            });
        }

        // key=value 解析
        let mut files = Vec::new();
        let mut output = None;
        let mut target = None;
        let mut opt_level = OptLevel::None;
        let mut sysroot = None;
        let mut crate_name = None;
        let mut debug_info = false;
        let mut emit_llvm_ir = false;
        let mut pretty = true;

        for arg in rest {
            let (key, value) = arg
                .split_once('=')
                .ok_or_else(|| CliError(format!("参数必须形如 `key=value`: `{arg}`")))?;

            match key {
                "file" => {
                    if value.is_empty() {
                        return Err(CliError("`file` 不能为空".into()));
                    }
                    for p in value.split(',') {
                        files.push(PathBuf::from(p));
                    }
                }
                "output" => output = Some(PathBuf::from(value)),
                "target" => target = Some(parse_target(value)?),
                "opt" => opt_level = parse_opt(value)?,
                "sysroot" => sysroot = Some(PathBuf::from(value)),
                "crate_name" => crate_name = Some(value.into()),
                "debug_info" => debug_info = parse_bool(value)?,
                "emit_llvm_ir" => emit_llvm_ir = parse_bool(value)?,
                "pretty" => pretty = parse_bool(value)?,
                _ => return Err(CliError(format!("未知参数: `{key}`"))),
            }
        }

        // 校验
        if files.is_empty() {
            return Err(CliError("缺少 `file=<path>`".into()));
        }

        // 默认值
        let target = target
            .or_else(TargetTriple::host)
            .ok_or_else(|| CliError("不支持的宿主平台，请显式指定 `target=`".into()))?;
        let sysroot = sysroot.unwrap_or_else(|| PathBuf::from("/"));

        let command = match command {
            Command::Build => Command::Build,
            other => other,
        };

        Ok(Cli {
            command,
            files,
            output,
            target,
            opt_level,
            sysroot,
            crate_name,
            debug_info,
            emit_llvm_ir,
            pretty,
        })
    }
}

fn parse_target(s: &str) -> Result<TargetTriple, CliError> {
    // 简化版: `x86_64-linux` 格式
    let (arch, os) = s
        .split_once('-')
        .ok_or_else(|| CliError(format!("`target` 需形如 `arch-os`: `{s}`")))?;

    use litec_session::{Arch, Os};

    let arch = match arch {
        "x86_64" => Arch::X86_64,
        "aarch64" => Arch::Aarch64,
        "riscv64" => Arch::Riscv64,
        _ => return Err(CliError(format!("未知架构: `{arch}`"))),
    };
    let os = match os {
        "linux" => Os::Linux,
        "windows" => Os::Windows,
        "macos" => Os::Macos,
        _ => return Err(CliError(format!("未知系统: `{os}`"))),
    };

    Ok(TargetTriple::new(arch, os))
}

fn parse_opt(s: &str) -> Result<OptLevel, CliError> {
    match s {
        "0" => Ok(OptLevel::None),
        "1" | "2" | "3" => Ok(OptLevel::Speed), // 简化
        "s" | "z" => Ok(OptLevel::Size),
        _ => Err(CliError(format!("无效的 `opt`: `{s}`"))),
    }
}

fn parse_bool(s: &str) -> Result<bool, CliError> {
    match s {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(CliError(format!("需要 true/false: `{s}`"))),
    }
}

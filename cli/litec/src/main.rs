//! litec —— Lite 编译器 CLI
//!
//! 纯参数驱动

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use litec_parse::tokenize;
use litec_session::{SessOptions, Session};
use litec_span::{SourceMap, create_session_globals};

mod cli;
use cli::{Cli, CliError, Command};

fn main() -> ExitCode {
    let cli = match Cli::parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(CliError(msg)) => {
            eprintln!("litec: {msg}");
            eprintln!();
            eprint!("{}", cli::HELP);
            return ExitCode::from(2);
        }
    };

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("litec: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Command::Help => {
            print!("{}", cli::HELP);
            Ok(())
        }
        Command::Version => {
            println!("litec {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Lex => lex(&cli, &cli.files),
        Command::Ast => ast(&cli, &cli.files),
        Command::Build => {
            lex(&cli, &cli.files)?;
            eprintln!("note: `build` 尚未实现(Parser 未完成)");
            Ok(())
        }
    }
}

fn ast(cli: &Cli, files: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    let source_map = Arc::new(SourceMap::new());

    create_session_globals(Some(source_map.clone()), &[], || {
        let opts = SessOptions {
            crate_name: cli
                .crate_name
                .clone()
                .unwrap_or_else(|| derive_crate_name(&files[0])),
            opt_level: cli.opt_level,
            target: cli.target.clone(),
            sysroot: cli.sysroot.clone(),
            debug_info: cli.debug_info,
            emit_llvm_ir: cli.emit_llvm_ir,
        };
        let session = Session::new(source_map.clone(), opts);

        for path in files {
            ast_one(&session, &source_map, path, cli.pretty)?;
        }

        let diags = session.take_diags();
        for d in &diags {
            eprint!("{}", d.render(&source_map));
        }
        if !diags.is_empty() {
            std::process::exit(1);
        }
        Ok(())
    })
}

fn ast_one(
    session: &Session,
    source_map: &Arc<SourceMap>,
    path: &PathBuf,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(path)?;
    let file_id = source_map.add_file(path.clone(), src.into());

    let krate = litec_parse::parse(session, file_id);

    println!("// {}: {} items", path.display(), krate.items.len());
    if pretty {
        println!("{}", krate);
    } else {
        println!("{:#?}", krate);
    }

    Ok(())
}

fn lex(cli: &Cli, files: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    let source_map = Arc::new(SourceMap::new());

    create_session_globals(Some(source_map.clone()), &[], || {
        let opts = SessOptions {
            crate_name: cli
                .crate_name
                .clone()
                .unwrap_or_else(|| derive_crate_name(&files[0])),
            opt_level: cli.opt_level,
            target: cli.target.clone(),
            sysroot: cli.sysroot.clone(),
            debug_info: cli.debug_info,
            emit_llvm_ir: cli.emit_llvm_ir,
        };
        let session = Session::new(source_map.clone(), opts);

        for path in files {
            lex_one(&session, &source_map, path)?;
        }

        // 输出诊断
        let diags = session.take_diags();
        for d in &diags {
            eprint!("{}", d.render(&source_map));
        }

        if !diags.is_empty() {
            std::process::exit(1);
        }
        Ok(())
    })
}

fn lex_one(
    session: &Session,
    source_map: &Arc<SourceMap>,
    path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(path)?;
    let file_id = source_map.add_file(path.clone(), src.into());

    let tokens = tokenize(session, file_id);

    println!("{}: {} tokens", path.display(), tokens.len());

    for token in &tokens {
        println!(
            "{:>5}..{:<5}{:<10}`{}`",
            token.span.start,
            token.span.end,
            format!("{:?}", token.kind),
            token.text.as_str(),
        );
    }

    Ok(())
}

fn derive_crate_name(path: &PathBuf) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main")
        .to_string()
}

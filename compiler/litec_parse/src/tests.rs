use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use litec_ast::TokenKind;
use litec_ast::token::Token;
use litec_error::Diag;
use litec_session::{SessOptions, Session, TargetTriple};
use litec_span::{FileId, SourceMap, create_session_globals};

use crate::lexer::Lexer;

/// 每个测试独立构造 `SessionGlobals`——测试之间不互相影响。
fn with_session<F, R>(f: F) -> R
where
    F: FnOnce(&Session) -> R,
{
    let source_map = Arc::new(SourceMap::new());

    create_session_globals(Some(source_map.clone()), &[], || {
        let opts = SessOptions::new(
            TargetTriple::host().expect("unsupported host"),
            PathBuf::from("/"),
        );
        f(&Session::new(source_map, opts))
    })
}

fn lex_with_diags(src: &str) -> (Vec<Token>, Vec<Diag>) {
    with_session(|session| {
        let file_id = session
            .source_map()
            .add_file(Path::new("test.lite").to_path_buf(), src);
        let mut lexer = Lexer::new(session, src, file_id);
        let mut toks = Vec::new();
        let mut err_count = 0;

        loop {
            match lexer.advance() {
                Ok(tok) => {
                    let is_eof = tok.kind == TokenKind::Eof;
                    toks.push(tok);
                    if is_eof {
                        break;
                    }
                }
                Err(_) => {
                    err_count += 1;
                    if err_count > 100 {
                        break;
                    }
                    continue;
                }
            }
        }

        let diags = session.take_diags();
        (toks, diags)
    })
}

/// 过滤 trivia（空白、注释）——大部分断言只关心"有意义的 token"。
fn significant(toks: Vec<Token>) -> Vec<Token> {
    toks.into_iter()
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment
                    | TokenKind::BlockComment
                    | TokenKind::DocLineComment
                    | TokenKind::DocBlockComment
                    | TokenKind::InnerDocLineComment
                    | TokenKind::InnerDocBlockComment
            )
        })
        .collect()
}

/// 断言无错误，返回 significant token（含 `Eof`）。
fn lex_ok(src: &str) -> Vec<Token> {
    let (toks, diags) = lex_with_diags(src);
    assert!(
        diags.is_empty(),
        "expected no diagnostics, got {}:\n{:#?}",
        diags.len(),
        diags,
    );
    significant(toks)
}

/// 断言有错误，返回诊断。
fn lex_err(src: &str) -> Vec<Diag> {
    let (_, diags) = lex_with_diags(src);
    assert!(!diags.is_empty(), "expected diagnostics, got none");
    diags
}

/// 只取 `TokenKind` 列表（`significant` 后）——便于 `assert_eq!`。
fn kinds(toks: &[Token]) -> Vec<TokenKind> {
    toks.iter().map(|t| t.kind).collect()
}

mod lexer_whitespace_and_comments {
    use super::*;

    #[test]
    fn empty_input() {
        let toks = lex_ok("");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, TokenKind::Eof);
    }

    #[test]
    fn only_whitespace() {
        let toks = lex_ok("   \n\t  ");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, TokenKind::Eof);
    }

    #[test]
    fn fullwidth_space_warns() {
        // `\u{3000}` 是空白——产出 Whitespace + 一条警告
        let (toks, diags) = lex_with_diags("let\u{3000}x");
        assert!(diags.iter().any(|d| d.message.contains("fullwidth")));
        assert!(toks.iter().any(|t| t.kind == TokenKind::Whitespace));
    }

    #[test]
    fn line_comment() {
        let toks = lex_ok("// hello\nfn");
        assert_eq!(kinds(&toks), vec![TokenKind::Fn, TokenKind::Eof]);
    }

    #[test]
    fn doc_line_comment() {
        let toks = lex_ok("/// doc\nfn");
        assert_eq!(kinds(&toks), vec![TokenKind::Fn, TokenKind::Eof]);
    }

    #[test]
    fn inner_doc_line_comment() {
        let toks = lex_ok("//! inner doc\nfn");
        assert_eq!(kinds(&toks), vec![TokenKind::Fn, TokenKind::Eof]);
    }

    #[test]
    fn block_comment() {
        let toks = lex_ok("/* hello */ fn");
        assert_eq!(kinds(&toks), vec![TokenKind::Fn, TokenKind::Eof]);
    }

    #[test]
    fn nested_block_comment() {
        let toks = lex_ok("/* outer /* inner */ still outer */ fn");
        assert_eq!(kinds(&toks), vec![TokenKind::Fn, TokenKind::Eof]);
    }

    #[test]
    fn doc_block_comment() {
        let toks = lex_ok("/** doc */ fn");
        assert_eq!(kinds(&toks), vec![TokenKind::Fn, TokenKind::Eof]);
    }

    #[test]
    fn empty_block_comment_is_plain() {
        // `/**/` 是普通块注释，不是 doc
        let (toks, diags) = lex_with_diags("/**/ fn");
        assert!(diags.is_empty());
        assert_eq!(
            kinds(&significant(toks)),
            vec![TokenKind::Fn, TokenKind::Eof]
        );
    }

    #[test]
    fn unterminated_block_comment() {
        let diags = lex_err("/* hello");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unterminated block comment"))
        );
    }
}

mod lexer_identifiers {
    use super::*;

    #[test]
    fn keywords() {
        let toks = lex_ok("fn let if else while return true false in");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::Fn,
                TokenKind::Let,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::While,
                TokenKind::Return,
                TokenKind::True,
                TokenKind::False,
                TokenKind::In,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn keywords_extended() {
        let toks = lex_ok("struct enum union trait impl type mod use");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::Struct,
                TokenKind::Enum,
                TokenKind::Union,
                TokenKind::Trait,
                TokenKind::Impl,
                TokenKind::Type,
                TokenKind::Mod,
                TokenKind::Use,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn self_keywords() {
        let toks = lex_ok("self Self super crate");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::SelfLower,
                TokenKind::SelfUpper,
                TokenKind::Super,
                TokenKind::Crate,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn plain_identifier() {
        let toks = lex_ok("foo");
        assert!(matches!(toks[0].kind, TokenKind::Ident));
        assert_eq!(toks[1].kind, TokenKind::Eof);
    }

    #[test]
    fn underscore_is_ident_start() {
        let toks = lex_ok("_x");
        assert!(matches!(toks[0].kind, TokenKind::Ident));
    }

    #[test]
    fn lone_underscore() {
        let toks = lex_ok("_");
        assert_eq!(toks[0].kind, TokenKind::Underscore);
    }

    #[test]
    fn cjk_identifier() {
        let toks = lex_ok("变量");
        assert!(matches!(toks[0].kind, TokenKind::Ident));
    }

    #[test]
    fn mixed_identifier() {
        let toks = lex_ok("foo_bar123");
        assert!(matches!(toks[0].kind, TokenKind::Ident));
    }

    #[test]
    fn keyword_prefix_is_ident() {
        // `fnord` 不是关键字——是标识符
        let toks = lex_ok("fnord");
        assert!(matches!(toks[0].kind, TokenKind::Ident));
    }
}

mod lexer_numbers {
    use super::*;
    use litec_ast::token::{LiteralKind, Radix};

    #[test]
    fn decimal_int() {
        let toks = lex_ok("42");
        match toks[0].kind {
            TokenKind::Literal {
                kind: LiteralKind::Integer { base: Radix::Dec },
                ..
            } => {}
            ref k => panic!("expected Dec integer, got {:?}", k),
        }
    }

    #[test]
    fn hex_int() {
        let toks = lex_ok("0xFF");
        match toks[0].kind {
            TokenKind::Literal {
                kind: LiteralKind::Integer { base: Radix::Hex },
                ..
            } => {}
            ref k => panic!("expected Hex, got {:?}", k),
        }
    }

    #[test]
    fn bin_int() {
        let toks = lex_ok("0b1010");
        match toks[0].kind {
            TokenKind::Literal {
                kind: LiteralKind::Integer { base: Radix::Bin },
                ..
            } => {}
            ref k => panic!("expected Bin, got {:?}", k),
        }
    }

    #[test]
    fn oct_int() {
        let toks = lex_ok("0o777");
        match toks[0].kind {
            TokenKind::Literal {
                kind: LiteralKind::Integer { base: Radix::Oct },
                ..
            } => {}
            ref k => panic!("expected Oct, got {:?}", k),
        }
    }

    #[test]
    fn float_simple() {
        let toks = lex_ok("3.14");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Float,
                ..
            }
        ));
    }

    #[test]
    fn float_leading_dot() {
        let toks = lex_ok(".5");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Float,
                ..
            }
        ));
    }

    #[test]
    fn float_exponent() {
        let toks = lex_ok("1e10");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Float,
                ..
            }
        ));
    }

    #[test]
    fn float_exponent_negative() {
        let toks = lex_ok("1.5e-3");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Float,
                ..
            }
        ));
    }

    #[test]
    fn int_with_underscores() {
        let toks = lex_ok("1_000_000");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Integer { .. },
                ..
            }
        ));
    }

    #[test]
    fn int_with_suffix() {
        let toks = lex_ok("42i32");
        match toks[0].kind {
            TokenKind::Literal {
                kind: LiteralKind::Integer { .. },
                suffix: Some(_),
            } => {}
            ref k => panic!("expected suffix, got {:?}", k),
        }
    }

    #[test]
    fn hex_with_suffix() {
        let toks = lex_ok("0xFFu32");
        match toks[0].kind {
            TokenKind::Literal {
                kind: LiteralKind::Integer { base: Radix::Hex },
                suffix: Some(_),
            } => {}
            ref k => panic!("expected hex+suffix, got {:?}", k),
        }
    }

    #[test]
    fn dot_after_int_not_float() {
        // `1.foo` 应该是 Int(1) + Dot + Ident(foo)
        let toks = lex_ok("1.foo");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Integer { .. },
                ..
            }
        ));
        assert_eq!(toks[1].kind, TokenKind::Dot);
        assert!(matches!(toks[2].kind, TokenKind::Ident));
    }
}

mod lexer_literals {
    use super::*;
    use litec_ast::token::LiteralKind;

    #[test]
    fn simple_string() {
        let toks = lex_ok(r#""hello""#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Str,
                ..
            }
        ));
    }

    #[test]
    fn string_with_escape() {
        let toks = lex_ok(r#""a\nb""#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Str,
                ..
            }
        ));
    }

    #[test]
    fn string_with_escaped_quote() {
        let toks = lex_ok(r#""a\"b""#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Str,
                ..
            }
        ));
    }

    #[test]
    fn string_with_unicode_escape() {
        let toks = lex_ok(r#""\u{4E2D}""#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Str,
                ..
            }
        ));
    }

    #[test]
    fn unicode_string_content() {
        let toks = lex_ok(r#""中文""#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Str,
                ..
            }
        ));
    }

    #[test]
    fn simple_char() {
        let toks = lex_ok("'a'");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Char,
                ..
            }
        ));
    }

    #[test]
    fn char_escape() {
        let toks = lex_ok(r"'\n'");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Char,
                ..
            }
        ));
    }

    #[test]
    fn char_escaped_quote() {
        let toks = lex_ok(r"'\''");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Char,
                ..
            }
        ));
    }

    #[test]
    fn unicode_char() {
        let toks = lex_ok("'中'");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Char,
                ..
            }
        ));
    }

    #[test]
    fn raw_string_backtick() {
        let toks = lex_ok("`hello world`");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::RawStr,
                ..
            }
        ));
    }

    #[test]
    fn raw_string_multiline() {
        let toks = lex_ok("`line1\nline2`");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::RawStr,
                ..
            }
        ));
    }

    #[test]
    fn raw_string_no_escape() {
        // raw 里 `\n` 是两个字面字符
        let toks = lex_ok(r"`a\nb`");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::RawStr,
                ..
            }
        ));
    }

    #[test]
    fn byte_literal() {
        let toks = lex_ok("b'a'");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::Byte,
                ..
            }
        ));
    }

    #[test]
    fn byte_string() {
        let toks = lex_ok(r#"b"hello""#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::BStr,
                ..
            }
        ));
    }

    #[test]
    fn byte_string_escapes() {
        let toks = lex_ok(r#"b"\xE4\xB8\xAD""#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::BStr,
                ..
            }
        ));
    }

    #[test]
    fn c_char() {
        let toks = lex_ok("c'a'");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::CChar,
                ..
            }
        ));
    }

    #[test]
    fn c_string() {
        let toks = lex_ok(r#"c"hello""#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Literal {
                kind: LiteralKind::CStr,
                ..
            }
        ));
    }

    #[test]
    fn b_and_c_alone_are_idents() {
        // `b` / `c` 后不跟引号——是标识符
        let toks = lex_ok("b c");
        assert!(matches!(toks[0].kind, TokenKind::Ident));
        assert!(matches!(toks[1].kind, TokenKind::Ident));
    }
}

mod lexer_punct {
    use super::*;

    #[test]
    fn single_char_punct() {
        let toks = lex_ok("( ) { } [ ] ; , @ # ~ ? $ :");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::OpenParen,
                TokenKind::CloseParen,
                TokenKind::OpenBrace,
                TokenKind::CloseBrace,
                TokenKind::OpenBracket,
                TokenKind::CloseBracket,
                TokenKind::Semi,
                TokenKind::Comma,
                TokenKind::At,
                TokenKind::Hash,
                TokenKind::Tilde,
                TokenKind::Question,
                TokenKind::Dollar,
                TokenKind::Colon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn multi_char_punct() {
        let toks = lex_ok(":: -> => == != <= >= .. ..= ...");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::PathAccess,
                TokenKind::Arrow,
                TokenKind::FatArrow,
                TokenKind::EqEq,
                TokenKind::NotEq,
                TokenKind::Le,
                TokenKind::Ge,
                TokenKind::To,
                TokenKind::ToEq,
                TokenKind::Ellipsis,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn arithmetic() {
        let toks = lex_ok("+ - * / % ^ & |");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Mul,
                TokenKind::Div,
                TokenKind::Remainder,
                TokenKind::BitXor,
                TokenKind::BitAnd,
                TokenKind::BitOr,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn compound_assign() {
        let toks = lex_ok("+= -= *= /= %= &= |= ^= <<= >>=");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::PlusEq,
                TokenKind::MinusEq,
                TokenKind::MulEq,
                TokenKind::DivEq,
                TokenKind::RemainderEq,
                TokenKind::BitAndEq,
                TokenKind::BitOrEq,
                TokenKind::BitXorEq,
                TokenKind::ShlEq,
                TokenKind::ShrEq,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn shifts_and_logic() {
        let toks = lex_ok("<< >> && ||");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::Shl,
                TokenKind::Shr,
                TokenKind::And,
                TokenKind::Or,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn longest_match() {
        // `>>=` 不应该是 `>` `>=`
        let toks = lex_ok(">>=");
        assert_eq!(kinds(&toks), vec![TokenKind::ShrEq, TokenKind::Eof]);
    }

    #[test]
    fn dot_vs_dotdot() {
        let toks = lex_ok(". .. ... ..=");
        assert_eq!(
            kinds(&toks),
            vec![
                TokenKind::Dot,
                TokenKind::To,
                TokenKind::Ellipsis,
                TokenKind::ToEq,
                TokenKind::Eof,
            ]
        );
    }
}

mod lexer_errors {
    use super::*;

    #[test]
    fn unknown_ascii() {
        // `\` 单独——不在 lex_punct 里
        let diags = lex_err(r"\");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unknown character"))
        );
    }

    #[test]
    fn emoji_is_unknown() {
        let diags = lex_err("let x = 😀;");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unknown character"))
        );
    }

    #[test]
    fn fullwidth_comma() {
        let diags = lex_err("let x = 1，2;");
        assert!(diags.iter().any(|d| d.message.contains("fullwidth")));
    }

    #[test]
    fn fullwidth_paren() {
        let diags = lex_err("fn main（) {}");
        assert!(diags.iter().any(|d| d.message.contains("fullwidth")));
    }

    #[test]
    fn unclosed_string() {
        let diags = lex_err(r#"let x = "hello"#);
        assert!(diags.iter().any(|d| d.message.contains("unclosed string")));
    }

    #[test]
    fn unclosed_char() {
        let diags = lex_err("let x = 'a");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unclosed character"))
        );
    }

    #[test]
    fn unclosed_raw_string() {
        let diags = lex_err("let x = `hello");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unclosed raw string"))
        );
    }

    #[test]
    fn non_ascii_in_byte_string() {
        let diags = lex_err(r#"let x = b"中";"#);
        assert!(diags.iter().any(|d| d.message.contains("non-ASCII")));
    }

    #[test]
    fn non_ascii_in_c_string() {
        let diags = lex_err(r#"let x = c"中";"#);
        assert!(diags.iter().any(|d| d.message.contains("non-ASCII")));
    }

    #[test]
    fn string_stops_at_newline() {
        // 未闭合的字符串遇到 `\n` 就停——后面的 `let y` 仍能解析
        let (toks, diags) = lex_with_diags("let x = \"hello\nlet y = 1;");
        assert!(!diags.is_empty());
        // 后面还有 token（不是全部吞掉）
        let has_let_after = toks.iter().skip(1).any(|t| t.kind == TokenKind::Let);
        assert!(has_let_after, "lexer should not consume past newline");
    }
}

mod lexer_spans {
    use super::*;

    #[test]
    fn ascii_spans_are_bytes() {
        let toks = lex_ok("fn main");
        assert_eq!(toks[0].span.start, 0);
        assert_eq!(toks[0].span.end, 2);
    }

    #[test]
    fn cjk_span_is_bytes() {
        // `变量` = 3 + 3 = 6 字节
        let toks = lex_ok("变量");
        assert_eq!(toks[0].span.start, 0);
        assert_eq!(toks[0].span.end, 6);
    }

    #[test]
    fn string_span_covers_quotes() {
        // `"中"` = 1 + 3 + 1 = 5 字节
        let toks = lex_ok("\"中\"");
        assert_eq!(toks[0].span.start, 0);
        assert_eq!(toks[0].span.end, 5);
    }

    #[test]
    fn emoji_span() {
        // 😀 = 4 字节
        let (_, diags) = lex_with_diags("😀");
        assert_eq!(diags[0].span.start, 0);
        assert_eq!(diags[0].span.end, 4);
    }

    #[test]
    fn fullwidth_span() {
        // ，= 3 字节
        let (_, diags) = lex_with_diags("，");
        assert_eq!(diags[0].span.end - diags[0].span.start, 3);
    }
}

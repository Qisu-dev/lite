//! Parser 测试

use std::path::PathBuf;
use std::sync::Arc;

use expect_test::expect;
use litec_ast::TokenKind;
use litec_ast::ast::{Item, Pat, Stmt, Ty};
use litec_session::{SessOptions, Session, TargetTriple};
use litec_span::{SourceMap, create_session_globals};

use crate::parser::ParseCtx;

fn test_session(source_map: Arc<SourceMap>) -> Session {
    Session::new(
        source_map,
        SessOptions::new("test", TargetTriple::host().unwrap(), PathBuf::from("/")),
    )
}

/// 统一入口——在 `create_session_globals` 里跑 `f`。
/// `f` 拿 `ParseCtx`，返回 `PResult<R>`。
fn check<R, F>(src: &str, f: F, expect: expect_test::Expect)
where
    R: std::fmt::Debug,
    F: for<'a, 'b> FnOnce(&mut ParseCtx<'a, 'b>) -> litec_error::PResult<R>,
{
    create_session_globals(None, &[], || {
        let source_map = Arc::new(SourceMap::new());
        let session = test_session(source_map.clone());
        let file_id = source_map.add_file("test.lite", src);
        let mut ctx = ParseCtx::new(&session, src, file_id);
        let r =
            f(&mut ctx).unwrap_or_else(|_| panic!("parse failed:\n{}", session.dcx().render_all()));
        expect.assert_eq(&format!("{r:#?}"));
    });
}

/// 文件级——容错解析，输出诊断 + items
fn check_file(src: &str, expect: expect_test::Expect) {
    create_session_globals(None, &[], || {
        let source_map = Arc::new(SourceMap::new());
        let session = test_session(source_map.clone());
        let file_id = source_map.add_file("test.lite", src);
        let mut ctx = ParseCtx::new(&session, src, file_id);
        let items = ctx
            .parse_list::<Item>(TokenKind::Eof, None)
            .unwrap_or_default();
        let diags = session.dcx().render_all();

        let mut out = String::new();
        if !diags.is_empty() {
            out.push_str(&diags);
            out.push('\n');
        }
        out.push_str(&format!("{items:#?}"));
        expect.assert_eq(&out);
    });
}

// ═══════════════════════════════════════════════════════════
// Expr
// ═══════════════════════════════════════════════════════════

mod expr {
    use super::*;

    #[test]
    fn literal_int() {
        check(
            "42",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr {
                node_id: NodeId(0),
                kind: Literal(Lit {
                    kind: Integer { base: Dec },
                    value: "42",
                    suffix: None,
                    span: 0..2,
                }),
                span: 0..2,
            }
        "#]],
        );
    }

    #[test]
    fn path_simple() {
        check(
            "foo",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... }
        "#]],
        );
    }

    #[test]
    fn binary_add() {
        check(
            "a + b",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Binary(a, Add, b) ... }
        "#]],
        );
    }

    #[test]
    fn precedence_mul_over_add() {
        // 1 + 2 * 3  →  1 + (2 * 3)
        check(
            "1 + 2 * 3",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Binary(1, Add, Binary(2, Mul, 3)) ... }
        "#]],
        );
    }

    #[test]
    fn precedence_assign_right() {
        // a = b = c  →  a = (b = c)
        check(
            "a = b = c",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Assign(a, Assign(b, c)) ... }
        "#]],
        );
    }

    #[test]
    fn postfix_call_chain() {
        check(
            "f(x).g(y)",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... }
        "#]],
        );
    }

    #[test]
    fn tuple_empty() {
        check(
            "()",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Unit ... }
        "#]],
        );
    }

    #[test]
    fn tuple_single_grouped() {
        check(
            "(1)",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Grouped(1) ... }
        "#]],
        );
    }

    #[test]
    fn tuple_single_with_comma() {
        check(
            "(1,)",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Tuple([1]) ... }
        "#]],
        );
    }

    #[test]
    fn array_literal() {
        check(
            "[1, 2, 3]",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Array([1, 2, 3]) ... }
        "#]],
        );
    }

    #[test]
    fn array_repeat() {
        check(
            "[0; 4]",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... ArrayRepeat(0, 4) ... }
        "#]],
        );
    }

    #[test]
    fn if_expr() {
        check(
            "if c { 1 } else { 2 }",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... If(c, Block([1]), Some(Block([2]))) ... }
        "#]],
        );
    }

    #[test]
    fn match_expr() {
        check(
            "match x { 0 => a, _ => b }",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Match(x, [...]) ... }
        "#]],
        );
    }

    #[test]
    fn closure_no_params() {
        check(
            "|| 1",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Closure([], 1) ... }
        "#]],
        );
    }

    #[test]
    fn closure_one_param() {
        check(
            "|x| x + 1",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Closure([x], Binary(x, Add, 1)) ... }
        "#]],
        );
    }

    #[test]
    fn cast() {
        check(
            "x as i32",
            |ctx| ctx.parse_expr(),
            expect![[r#"
            Expr { ... Cast(x, i32) ... }
        "#]],
        );
    }
}

// ═══════════════════════════════════════════════════════════
// Ty
// ═══════════════════════════════════════════════════════════

mod ty {
    use super::*;

    #[test]
    fn path_simple() {
        check(
            "i32",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Path(i32) ... }
        "#]],
        );
    }

    #[test]
    fn path_generic() {
        check(
            "Vec<i32>",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... }
        "#]],
        );
    }

    #[test]
    fn path_generic_nested_shr() {
        // Vec<Vec<i32>> —— `>>` 要拆
        check(
            "Vec<Vec<i32>>",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... }
        "#]],
        );
    }

    #[test]
    fn ptr_immut() {
        check(
            "*i32",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Ptr(Immut, i32) ... }
        "#]],
        );
    }

    #[test]
    fn ptr_mut() {
        check(
            "*mut i32",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Ptr(Mut, i32) ... }
        "#]],
        );
    }

    #[test]
    fn slice() {
        check(
            "[i32]",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Slice(i32) ... }
        "#]],
        );
    }

    #[test]
    fn array() {
        check(
            "[i32; 4]",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Array { elem: i32, len: Expr(4) } ... }
        "#]],
        );
    }

    #[test]
    fn array_infer() {
        check(
            "[i32; _]",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Array { elem: i32, len: Infer } ... }
        "#]],
        );
    }

    #[test]
    fn tuple() {
        check(
            "(i32, u8)",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Tuple([i32, u8]) ... }
        "#]],
        );
    }

    #[test]
    fn unit() {
        check(
            "()",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Unit ... }
        "#]],
        );
    }

    #[test]
    fn fn_ptr() {
        check(
            "fn(i32) -> u8",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... FnPtr([i32], Some(u8)) ... }
        "#]],
        );
    }

    #[test]
    fn dyn_single() {
        check(
            "*dyn Display",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Ptr(Immut, Dyn([Display])) ... }
        "#]],
        );
    }

    #[test]
    fn dyn_multi() {
        check(
            "*dyn Display + Send",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Ptr(Immut, Dyn([Display, Send])) ... }
        "#]],
        );
    }

    #[test]
    fn never() {
        check(
            "!",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Never ... }
        "#]],
        );
    }

    #[test]
    fn infer() {
        check(
            "_",
            |ctx| ctx.parse::<Ty>(),
            expect![[r#"
            Ty { ... Infer ... }
        "#]],
        );
    }
}

// ═══════════════════════════════════════════════════════════
// Pat
// ═══════════════════════════════════════════════════════════

mod pat {
    use super::*;

    #[test]
    fn wild() {
        check(
            "_",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Wild ... }
        "#]],
        );
    }

    #[test]
    fn ident() {
        check(
            "x",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Ident(Immut, x) ... }
        "#]],
        );
    }

    #[test]
    fn ident_mut() {
        check(
            "mut x",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Ident(Mut, x) ... }
        "#]],
        );
    }

    #[test]
    fn literal() {
        check(
            "42",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Expr(Literal(42)) ... }
        "#]],
        );
    }

    #[test]
    fn tuple() {
        check(
            "(a, b)",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Tuple([a, b]) ... }
        "#]],
        );
    }

    #[test]
    fn struct_shorthand() {
        check(
            "Point { x, y }",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... }
        "#]],
        );
    }

    #[test]
    fn struct_with_rest() {
        check(
            "Point { x, .. }",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... }
        "#]],
        );
    }

    #[test]
    fn enum_unit() {
        check(
            "None",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Enum(None, None) ... }
        "#]],
        );
    }

    #[test]
    fn enum_tuple_single() {
        check(
            "Some(x)",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... }
        "#]],
        );
    }

    #[test]
    fn range_inclusive() {
        check(
            "1..=5",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Range(Some(1), Some(5), Closed) ... }
        "#]],
        );
    }

    #[test]
    fn range_suffix() {
        check(
            "1..",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Range(Some(1), None, HalfOpen) ... }
        "#]],
        );
    }

    #[test]
    fn range_prefix() {
        check(
            "..=5",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Range(None, Some(5), Closed) ... }
        "#]],
        );
    }

    #[test]
    fn or_pattern() {
        check(
            "1 | 2 | 3",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Or([1, 2, 3]) ... }
        "#]],
        );
    }

    #[test]
    fn or_pattern_leading_pipe() {
        check(
            "| 1 | 2",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Or([1, 2]) ... }
        "#]],
        );
    }

    #[test]
    fn slice_with_rest() {
        check(
            "[a, b, ..]",
            |ctx| ctx.parse::<litec_ast::ast::Pat>(),
            expect![[r#"
            Pat { ... Slice([a, b, Rest]) ... }
        "#]],
        );
    }
}

// ═══════════════════════════════════════════════════════════
// Stmt
// ═══════════════════════════════════════════════════════════

mod stmt {
    use super::*;

    #[test]
    fn expr_stmt_semi() {
        check(
            "1;",
            |ctx| ctx.parse::<Stmt>(),
            expect![[r#"
            Stmt { ... Semi(1) ... }
        "#]],
        );
    }

    #[test]
    fn expr_stmt_no_semi() {
        check(
            "1",
            |ctx| ctx.parse::<Stmt>(),
            expect![[r#"
            Stmt { ... Expr(1) ... }
        "#]],
        );
    }

    #[test]
    fn let_simple() {
        check(
            "let x = 1;",
            |ctx| ctx.parse::<Stmt>(),
            expect![[r#"
            Stmt { ... Let(x, None, Some(1)) ... }
        "#]],
        );
    }

    #[test]
    fn let_with_ty() {
        check(
            "let x: i32 = 1;",
            |ctx| ctx.parse::<Stmt>(),
            expect![[r#"
            Stmt { ... Let(x, Some(i32), Some(1)) ... }
        "#]],
        );
    }

    #[test]
    fn let_no_init() {
        check(
            "let x: i32;",
            |ctx| ctx.parse::<Stmt>(),
            expect![[r#"
            Stmt { ... Let(x, Some(i32), None) ... }
        "#]],
        );
    }

    #[test]
    fn let_destructure() {
        check(
            "let Point { x, y } = p;",
            |ctx| ctx.parse::<Stmt>(),
            expect![[r#"
            Stmt { ... Let(Struct(...), None, Some(Path(p))) ... }
        "#]],
        );
    }

    #[test]
    fn defer_simple() {
        check(
            "defer cleanup();",
            |ctx| ctx.parse::<Stmt>(),
            expect![[r#"
            Stmt { ... Defer(Call(cleanup, [])) ... }
        "#]],
        );
    }
}

// ═══════════════════════════════════════════════════════════
// Item
// ═══════════════════════════════════════════════════════════

mod item {
    use super::*;

    #[test]
    fn empty_fn() {
        check_file(
            "fn main() {}",
            expect![[r#"
            [
                Item {
                    node_id: NodeId(0),
                    attr: [],
                    visibility: Inherited,
                    span: 0..12,
                    kind: Fn(Fn {
                        node_id: NodeId(...),
                        sig: FnSig {
                            name: Ident { text: "main", span: 3..7 },
                            generics: Generics { params: [], ... },
                            params: [],
                            return_type: Default(...),
                            is_variadic: false,
                            abi: None,
                        },
                        body: Some(Block { stmts: [], ... }),
                    }),
                },
            ]
        "#]],
        );
    }

    #[test]
    fn fn_with_params() {
        check_file(
            "fn add(a: i32, b: i32) -> i32 { a }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn fn_generic() {
        check_file(
            "fn id<T>(x: T) -> T { x }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn struct_record() {
        check_file(
            "struct Point { x: i32, y: i32 }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn struct_tuple() {
        check_file(
            "struct Pair(i32, i32);",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn struct_unit() {
        check_file(
            "struct Marker;",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn enum_simple() {
        check_file(
            "enum Color { Red, Green, Blue }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn enum_with_data() {
        check_file(
            "enum Shape { Circle(f64), Rect { w: f64, h: f64 } }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn trait_simple() {
        check_file(
            "trait Display { fn fmt(self) -> String; }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn impl_inherent() {
        check_file(
            "impl Point { fn new() -> Self { todo!() } }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn impl_trait() {
        check_file(
            "impl Display for Point { fn fmt(self) -> String { todo!() } }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn use_simple() {
        check_file(
            "use foo::bar;",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn use_nested() {
        check_file(
            "use foo::{bar, baz};",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn use_glob() {
        check_file(
            "use foo::*;",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn const_item() {
        check_file(
            "const MAX: i32 = 100;",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    #[test]
    fn mod_inline() {
        check_file(
            "mod foo { fn bar() {} }",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }

    // ─── Recovery ──────────────────────────────────────────

    #[test]
    fn recover_bad_fn() {
        check_file(
            "fn good1() {}\nfn bad( { }\nfn good2() {}",
            expect![[r#"
                // error: expected ...
                [
                    Item { ... },
                    Item { ... },
                ]
            "#]],
        );
    }

    #[test]
    fn recover_missing_item_keyword() {
        check_file(
            "fn good() {}\n???\nfn also_good() {}",
            expect![[r#"
                // 1 diagnostics
                [
                    Item { ... },
                    Item { ... },
                ]
            "#]],
        );
    }

    #[test]
    fn end_to_end_hello() {
        check_file(
            "fn main() {\n    let x = 1;\n    let y = 2;\n    x + y;\n}",
            expect![[r#"
                [Item { ... }]
            "#]],
        );
    }
}

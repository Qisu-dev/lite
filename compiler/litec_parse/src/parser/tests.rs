//! Parser 测试 —— 一个测试函数，紧凑 snapshot。

use std::path::PathBuf;
use std::sync::Arc;

use expect_test::expect;
use litec_ast::TokenKind;
use litec_ast::ast::*;
use litec_session::{SessOptions, Session, TargetTriple};
use litec_span::{SourceMap, create_session_globals};

use crate::parser::ParseCtx;

fn test_session(sm: Arc<SourceMap>) -> Session {
    Session::new(
        sm,
        SessOptions::new("test", TargetTriple::host().unwrap(), PathBuf::new()),
    )
}

fn check<R, F>(src: &str, f: F, expect: expect_test::Expect)
where
    R: Dump,
    F: for<'a, 'b> FnOnce(&mut ParseCtx<'a, 'b>) -> litec_error::PResult<R>,
{
    create_session_globals(None, &[], || {
        let sm = Arc::new(SourceMap::new());
        let session = test_session(sm.clone());
        let file_id = sm.add_file("test.lite", src);
        let mut ctx = ParseCtx::new(&session, src, file_id);
        let r =
            f(&mut ctx).unwrap_or_else(|_| panic!("parse failed:\n{}", session.dcx().render_all()));
        expect.assert_eq(&r.dump());
    });
}

fn check_file(src: &str, expect: expect_test::Expect) {
    create_session_globals(None, &[], || {
        let sm = Arc::new(SourceMap::new());
        let session = test_session(sm.clone());
        let file_id = sm.add_file("test.lite", src);
        let mut ctx = ParseCtx::new(&session, src, file_id);
        let items = ctx
            .parse_list::<Item>(TokenKind::Eof, None)
            .unwrap_or_default();
        let diag_count = session.dcx().len();
        let mut out = String::new();
        if diag_count > 0 {
            out.push_str(&format!("// {} diagnostics\n", diag_count));
        }
        out.push_str(&dump_items(&items));
        expect.assert_eq(&out);
    });
}

pub trait Dump {
    fn dump(&self) -> String;
}

fn dump_items(items: &[Item]) -> String {
    items
        .iter()
        .map(|i| i.dump())
        .collect::<Vec<_>>()
        .join("\n")
}

fn dump_path(p: &Path) -> String {
    let segs: Vec<String> = p.segments.iter().map(dump_seg).collect();
    let joined = segs.join("::");
    match &p.qself {
        Some(q) => format!("<{} as {}>::{}", q.ty.dump(), dump_path(&q.trait_), joined),
        None => joined,
    }
}

fn dump_seg(s: &PathSegment) -> String {
    match &s.generic_args {
        Some(args) => {
            let inner: Vec<String> = args.args.iter().map(dump_generic_arg).collect();
            format!("{}<{}>", s.name.text.as_str(), inner.join(", "))
        }
        None => s.name.text.as_str().to_string(),
    }
}

fn dump_generic_arg(a: &GenericArg) -> String {
    match &a.kind {
        GenericArgKind::Type(t) => t.dump(),
        GenericArgKind::Const(e) => e.dump(),
    }
}

impl Dump for Ty {
    fn dump(&self) -> String {
        match &self.kind {
            TyKind::Path(p) => dump_path(p),
            TyKind::Never => "!".into(),
            TyKind::Unit => "()".into(),
            TyKind::Ptr(m, inner) => {
                let prefix = match m {
                    Mutability::Mut => "*mut ",
                    Mutability::Immut => "*",
                };
                format!("{}{}", prefix, inner.dump())
            }
            TyKind::Array { elem, len } => {
                let len_s = match len {
                    ArrayLen::Infer => "_".into(),
                    ArrayLen::Expr(e) => e.dump(),
                };
                format!("[{}; {}]", elem.dump(), len_s)
            }
            TyKind::Slice(inner) => format!("[{}]", inner.dump()),
            TyKind::Tuple(elems) => format!(
                "({})",
                elems
                    .iter()
                    .map(|t| t.dump())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TyKind::FnPtr(params, ret) => {
                let p: Vec<String> = params.iter().map(|t| t.dump()).collect();
                match ret {
                    Some(r) => format!("fn({}) -> {}", p.join(", "), r.dump()),
                    None => format!("fn({})", p.join(", ")),
                }
            }
            TyKind::SelfTy => "Self".into(),
            TyKind::Infer => "_".into(),
            TyKind::Dyn(paths) => format!(
                "dyn {}",
                paths.iter().map(dump_path).collect::<Vec<_>>().join(" + ")
            ),
        }
    }
}

impl Dump for Expr {
    fn dump(&self) -> String {
        match &self.kind {
            ExprKind::Literal(l) => l.value.as_str().to_string(),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Path(p) => dump_path(p),
            ExprKind::Binary(l, op, r) => {
                format!("({} {} {})", l.dump(), binop_str(&op.value), r.dump())
            }
            ExprKind::Unary(op, e) => format!("({}{})", unop_str(op), e.dump()),
            ExprKind::Grouped(e) => format!("({})", e.dump()),
            ExprKind::Assign(l, r) => format!("({} = {})", l.dump(), r.dump()),
            ExprKind::AssignOp(l, op, r) => {
                format!("({} {}= {})", l.dump(), assignop_str(&op.value), r.dump())
            }
            ExprKind::Call(f, args) => {
                let a: Vec<String> = args.iter().map(|e| e.dump()).collect();
                format!("{}({})", f.dump(), a.join(", "))
            }
            ExprKind::Block(_) => "{ ... }".into(),
            ExprKind::If(c, _, _) => format!("if {}", c.dump()),
            ExprKind::While(c, _) => format!("while {}", c.dump()),
            ExprKind::For { variable, iter, .. } => {
                format!("for {} in {}", variable.dump(), iter.dump())
            }
            ExprKind::Index(b, i) => format!("{}[{}]", b.dump(), i.dump()),
            ExprKind::Range(l, r, _) => format!(
                "{}..{}",
                l.as_ref().map(|e| e.dump()).unwrap_or_default(),
                r.as_ref().map(|e| e.dump()).unwrap_or_default()
            ),
            ExprKind::Loop(_) => "loop { ... }".into(),
            ExprKind::Field(b, n) => format!("{}.{}", b.dump(), n.text.as_str()),
            ExprKind::Tuple(elems) => format!(
                "({})",
                elems
                    .iter()
                    .map(|e| e.dump())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ExprKind::Unit => "()".into(),
            ExprKind::AddressOf(m, e) => {
                let p = match m {
                    Mutability::Mut => "&mut ",
                    Mutability::Immut => "&",
                };
                format!("{}{}", p, e.dump())
            }
            ExprKind::Deref(e) => format!("*{}", e.dump()),
            ExprKind::StructExpr(s) => {
                let f: Vec<String> = s
                    .fields
                    .iter()
                    .map(|f| {
                        if f.is_shorthand {
                            f.name.text.as_str().to_string()
                        } else {
                            format!("{}: {}", f.name.text.as_str(), f.value.dump())
                        }
                    })
                    .collect();
                format!("{} {{ {} }}", dump_path(&s.path), f.join(", "))
            }
            ExprKind::Cast(e, t) => format!("({} as {})", e.dump(), t.dump()),
            ExprKind::Match(_, _) => "match { ... }".into(),
            ExprKind::Return(Some(e)) => format!("return {}", e.dump()),
            ExprKind::Return(None) => "return".into(),
            ExprKind::Continue => "continue".into(),
            ExprKind::Break(Some(e)) => format!("break {}", e.dump()),
            ExprKind::Break(None) => "break".into(),
            ExprKind::Undefined => "undefined".into(),
            ExprKind::MatchBool { pattern, matched } => {
                format!("let {} = {}", pattern.dump(), matched.dump())
            }
            ExprKind::Underscore => "_".into(),
            ExprKind::Closure(params, body) => {
                let p: Vec<String> = params
                    .iter()
                    .map(|c| c.name.text.as_str().to_string())
                    .collect();
                format!("|{}| {}", p.join(", "), body.dump())
            }
            ExprKind::Try(e) => format!("{}?", e.dump()),
            ExprKind::Array(elems) => format!(
                "[{}]",
                elems
                    .iter()
                    .map(|e| e.dump())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ExprKind::ArrayRepeat(v, n) => format!("[{}; {}]", v.dump(), n.dump()),
            ExprKind::Let(p, e) => format!("let {} = {}", p.dump(), e.dump()),
        }
    }
}

fn binop_str(op: &BinOpKind) -> &'static str {
    use BinOpKind::*;
    match op {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Rem => "%",
        And => "&&",
        Or => "||",
        BitXor => "^",
        BitAnd => "&",
        BitOr => "|",
        Shl => "<<",
        Shr => ">>",
        Eq => "==",
        Ne => "!=",
        Lt => "<",
        Le => "<=",
        Gt => ">",
        Ge => ">=",
    }
}

fn unop_str(op: &UnOp) -> &'static str {
    match op {
        UnOp::Not => "!",
        UnOp::BitNot => "~",
        UnOp::Neg => "-",
    }
}

fn assignop_str(op: &AssignOpKind) -> &'static str {
    use AssignOpKind::*;
    match op {
        AddAssign => "+=",
        SubAssign => "-=",
        MulAssign => "*=",
        DivAssign => "/=",
        RemAssign => "%=",
        BitXorAssign => "^=",
        BitAndAssign => "&=",
        BitOrAssign => "|=",
        ShlAssign => "<<=",
        ShrAssign => ">>=",
    }
}

// ─── Pat ───────────────────────────────────────────────────

impl Dump for Pat {
    fn dump(&self) -> String {
        match &self.kind {
            PatKind::Wild => "_".into(),
            PatKind::Ident(m, n) => {
                let p = match m {
                    Mutability::Mut => "mut ",
                    Mutability::Immut => "",
                };
                format!("{}{}", p, n.text.as_str())
            }
            PatKind::Tuple(elems) => format!(
                "({})",
                elems
                    .iter()
                    .map(|p| p.dump())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PatKind::Struct(path, fields, rest) => {
                let mut fs: Vec<String> = fields
                    .iter()
                    .map(|f| {
                        if is_shorthand(&f.name, &f.pat) {
                            f.name.text.as_str().to_string()
                        } else {
                            format!("{}: {}", f.name.text.as_str(), f.pat.dump())
                        }
                    })
                    .collect();
                if *rest {
                    fs.push("..".into());
                }
                format!("{} {{ {} }}", dump_path(path), fs.join(", "))
            }
            PatKind::Enum(path, payload) => match payload {
                Some(p) => format!("{}({})", dump_path(path), p.dump()),
                None => dump_path(path),
            },
            PatKind::Expr(e) => e.dump(),
            PatKind::Range(l, r, _) => format!(
                "{}..{}",
                l.as_ref().map(|e| e.dump()).unwrap_or_default(),
                r.as_ref().map(|e| e.dump()).unwrap_or_default()
            ),
            PatKind::Rest => "..".into(),
            PatKind::Slice(elems) => format!(
                "[{}]",
                elems
                    .iter()
                    .map(|p| p.dump())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PatKind::Or(pats) => pats
                .iter()
                .map(|p| p.dump())
                .collect::<Vec<_>>()
                .join(" | "),
        }
    }
}

fn is_shorthand(name: &Ident, pat: &Pat) -> bool {
    matches!(&pat.kind, PatKind::Ident(_, n) if n.text == name.text)
}

// ─── Stmt ──────────────────────────────────────────────────

impl Dump for Stmt {
    fn dump(&self) -> String {
        match &self.kind {
            StmtKind::Expr(e) => e.dump(),
            StmtKind::Semi(e) => format!("{};", e.dump()),
            StmtKind::Let(p, ty, init) => {
                let ty_s = ty
                    .as_ref()
                    .map(|t| format!(": {}", t.dump()))
                    .unwrap_or_default();
                let init_s = init
                    .as_ref()
                    .map(|e| format!(" = {}", e.dump()))
                    .unwrap_or_default();
                format!("let {}{}{};", p.dump(), ty_s, init_s)
            }
            StmtKind::Defer(e) => format!("defer {};", e.dump()),
            StmtKind::Item(i) => i.dump(),
        }
    }
}

// ─── Item ──────────────────────────────────────────────────

impl Dump for Item {
    fn dump(&self) -> String {
        match &self.kind {
            ItemKind::Fn(f) => dump_fn(f),
            ItemKind::Struct(s) => {
                format!(
                    "struct {}{}",
                    s.name.text.as_str(),
                    dump_variant_data(&s.kind)
                )
            }
            ItemKind::Enum(name, _, variants) => {
                let vs: Vec<String> = variants.iter().map(dump_variant).collect();
                format!("enum {} {{ {} }}", name.text.as_str(), vs.join(", "))
            }
            ItemKind::Union(u) => {
                format!(
                    "union {}{}",
                    u.name.text.as_str(),
                    dump_variant_data(&u.kind)
                )
            }
            ItemKind::Trait(name, _, items) => {
                format!("trait {} {{ {} items }}", name.text.as_str(), items.len())
            }
            ItemKind::Impl(i) => match &i.of_trait {
                Some(t) => format!("impl {} for {}", dump_path(t), i.self_ty.dump()),
                None => format!("impl {}", i.self_ty.dump()),
            },
            ItemKind::Use(tree) => format!("use {}", dump_use_tree(tree)),
            ItemKind::Module(name, kind) => match kind {
                Inline::Inline(_) => format!("mod {} {{...}}", name.text.as_str()),
                Inline::External(_) => format!("mod {};", name.text.as_str()),
            },
            ItemKind::Const(name, ty, init) => {
                format!(
                    "const {}: {} = {}",
                    name.text.as_str(),
                    ty.dump(),
                    init.dump()
                )
            }
            ItemKind::Static(_, name, ty, init) => {
                format!(
                    "static {}: {} = {}",
                    name.text.as_str(),
                    ty.dump(),
                    init.dump()
                )
            }
            ItemKind::TypeAlias(t) => {
                format!("type {} = {}", t.name.text.as_str(), t.ty.dump())
            }
            ItemKind::Extern(e) => format!("extern {{ {} items }}", e.items.len()),
        }
    }
}

fn dump_fn(f: &Fn) -> String {
    let sig = &f.sig;
    let generics = if sig.generics.params.is_empty() {
        String::new()
    } else {
        let g: Vec<String> = sig
            .generics
            .params
            .iter()
            .map(|p| p.name.text.as_str().to_string())
            .collect();
        format!("<{}>", g.join(", "))
    };
    let params: Vec<String> = sig
        .params
        .iter()
        .map(|p| match &p.kind {
            ParamKind::Normal(pat, ty) => format!("{}: {}", pat.dump(), ty.dump()),
            ParamKind::SelfValue(_) => "self".into(),
            ParamKind::SelfPtr(_) => "*self".into(),
        })
        .collect();
    let ret = match &sig.return_type {
        FnRetTy::Ty(t) => format!(" -> {}", t.dump()),
        FnRetTy::Default(_) => String::new(),
    };
    let body = if f.body.is_some() { " { ... }" } else { ";" };
    format!(
        "fn {}{}({}){}{}",
        sig.name.text.as_str(),
        generics,
        params.join(", "),
        ret,
        body
    )
}

fn dump_variant_data(v: &VariantData) -> String {
    match v {
        VariantData::Unit => ";".into(),
        VariantData::Tuple(tys) => format!(
            "({});",
            tys.iter().map(|t| t.dump()).collect::<Vec<_>>().join(", ")
        ),
        VariantData::Struct(fields) => {
            let f: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name.text.as_str(), f.ty.dump()))
                .collect();
            format!(" {{ {} }}", f.join(", "))
        }
    }
}

fn dump_variant(v: &Variant) -> String {
    match &v.data {
        VariantData::Unit => v.ident.text.as_str().to_string(),
        VariantData::Tuple(tys) => format!(
            "{}({})",
            v.ident.text.as_str(),
            tys.iter().map(|t| t.dump()).collect::<Vec<_>>().join(", ")
        ),
        VariantData::Struct(fields) => {
            let f: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name.text.as_str(), f.ty.dump()))
                .collect();
            format!("{} {{ {} }}", v.ident.text.as_str(), f.join(", "))
        }
    }
}

fn dump_use_tree(tree: &UseTree) -> String {
    let prefix = dump_path(&tree.prefix);
    match &tree.kind {
        UseTreeKind::Simple(rename) => match rename {
            Some(r) => format!("{} as {}", prefix, r.text.as_str()),
            None => prefix,
        },
        UseTreeKind::Glob => format!("{}::*", prefix),
        UseTreeKind::Nested(trees, _) => {
            let inner: Vec<String> = trees.iter().map(dump_use_tree).collect();
            format!("{}::{{{}}}", prefix, inner.join(", "))
        }
    }
}

#[test]
fn expr_literals_and_paths() {
    check("42", |c| c.parse_expr(), expect!["42"]);
    check(r#""hi""#, |c| c.parse_expr(), expect![r#""hi""#]);
    check("foo", |c| c.parse_expr(), expect!["foo"]);
    check(
        "foo::bar::baz",
        |c| c.parse_expr(),
        expect!["foo::bar::baz"],
    );
}

#[test]
fn expr_binary_precedence() {
    check("a + b", |c| c.parse_expr(), expect!["(a + b)"]);
    check("1 + 2 * 3", |c| c.parse_expr(), expect!["(1 + (2 * 3))"]);
    check(
        "(1 + 2) * 3",
        |c| c.parse_expr(),
        expect!["(((1 + 2)) * 3)"],
    );
    check(
        "a && b || c",
        |c| c.parse_expr(),
        expect!["((a && b) || c)"],
    );
    check("1 == 2", |c| c.parse_expr(), expect!["(1 == 2)"]);
    check("1 < 2", |c| c.parse_expr(), expect!["(1 < 2)"]);
    check("1 + 2 + 3", |c| c.parse_expr(), expect!["((1 + 2) + 3)"]);
}

#[test]
fn expr_assign() {
    check("a = b = c", |c| c.parse_expr(), expect!["(a = (b = c))"]);
}

#[test]
fn expr_unary_and_postfix() {
    check("*p", |c| c.parse_expr(), expect!["*p"]);
    check("&x", |c| c.parse_expr(), expect!["&x"]);
    check("&mut x", |c| c.parse_expr(), expect!["&mut x"]);
    check("!x", |c| c.parse_expr(), expect!["(!x)"]);
    check("-x", |c| c.parse_expr(), expect!["(-x)"]);
    check("a.b.c", |c| c.parse_expr(), expect!["a.b.c"]);
    check("a[0]", |c| c.parse_expr(), expect!["a[0]"]);
    check("f()", |c| c.parse_expr(), expect!["f()"]);
    check("f(1, 2)", |c| c.parse_expr(), expect!["f(1, 2)"]);
    check("f(x).g(y)", |c| c.parse_expr(), expect!["f(x).g(y)"]);
    check("x as i32", |c| c.parse_expr(), expect!["(x as i32)"]);
}

#[test]
fn expr_tuple_unit_grouped() {
    check("()", |c| c.parse_expr(), expect!["()"]);
    check("(1)", |c| c.parse_expr(), expect!["(1)"]);
    check("(1,)", |c| c.parse_expr(), expect!["(1)"]);
    check("(1, 2)", |c| c.parse_expr(), expect!["(1, 2)"]);
}

#[test]
fn expr_array() {
    check("[1, 2, 3]", |c| c.parse_expr(), expect!["[1, 2, 3]"]);
    check("[0; 4]", |c| c.parse_expr(), expect!["[0; 4]"]);
}

#[test]
fn expr_closure() {
    check("|| 1", |c| c.parse_expr(), expect!["|| 1"]);
    check("|x| x + 1", |c| c.parse_expr(), expect!["|x| (x + 1)"]);
}

#[test]
fn ty_path() {
    check("i32", |c| c.parse::<Ty>(), expect!["i32"]);
    check("a::b::C", |c| c.parse::<Ty>(), expect!["a::b::C"]);
}

#[test]
fn ty_generic() {
    check("Vec<i32>", |c| c.parse::<Ty>(), expect!["Vec<i32>"]);
    check(
        "Vec<Vec<i32>>",
        |c| c.parse::<Ty>(),
        expect!["Vec<Vec<i32>>"],
    );
    check(
        "HashMap<K, V>",
        |c| c.parse::<Ty>(),
        expect!["HashMap<K, V>"],
    );
}

#[test]
fn ty_ptr() {
    check("*i32", |c| c.parse::<Ty>(), expect!["*i32"]);
    check("*mut i32", |c| c.parse::<Ty>(), expect!["*mut i32"]);
    check("**i32", |c| c.parse::<Ty>(), expect!["**i32"]);
}

#[test]
fn ty_slice_array() {
    check("[i32]", |c| c.parse::<Ty>(), expect!["[i32]"]);
    check("[i32; 4]", |c| c.parse::<Ty>(), expect!["[i32; 4]"]);
    check("[i32; _]", |c| c.parse::<Ty>(), expect!["[i32; _]"]);
}

#[test]
fn ty_tuple_unit() {
    check("(i32, u8)", |c| c.parse::<Ty>(), expect!["(i32, u8)"]);
    check("()", |c| c.parse::<Ty>(), expect!["()"]);
    check("(i32,)", |c| c.parse::<Ty>(), expect!["(i32)"]);
}

#[test]
fn ty_fn_ptr() {
    check(
        "fn(i32) -> u8",
        |c| c.parse::<Ty>(),
        expect!["fn(i32) -> u8"],
    );
    check("fn()", |c| c.parse::<Ty>(), expect!["fn()"]);
}

#[test]
fn ty_dyn() {
    check("*dyn Display", |c| c.parse::<Ty>(), expect!["*dyn Display"]);
    check(
        "*dyn Display + Send",
        |c| c.parse::<Ty>(),
        expect!["*dyn Display + Send"],
    );
}

#[test]
fn ty_never_infer() {
    check("!", |c| c.parse::<Ty>(), expect!["!"]);
    check("_", |c| c.parse::<Ty>(), expect!["_"]);
}

#[test]
fn pat_atoms() {
    check("_", |c| c.parse::<Pat>(), expect!["_"]);
    check("x", |c| c.parse::<Pat>(), expect!["x"]);
    check("mut x", |c| c.parse::<Pat>(), expect!["mut x"]);
    check("42", |c| c.parse::<Pat>(), expect!["42"]);
}

#[test]
fn pat_tuple_slice() {
    check("(a, b)", |c| c.parse::<Pat>(), expect!["(a, b)"]);
    check("[a, b, ..]", |c| c.parse::<Pat>(), expect!["[a, b, ..]"]);
    check("[]", |c| c.parse::<Pat>(), expect!["[]"]);
}

#[test]
fn pat_struct() {
    check(
        "Point { x, y }",
        |c| c.parse::<Pat>(),
        expect!["Point { x, y }"],
    );
    check(
        "Point { x, .. }",
        |c| c.parse::<Pat>(),
        expect!["Point { x, .. }"],
    );
    check(
        "Point { x: a, y: b }",
        |c| c.parse::<Pat>(),
        expect!["Point { x: a, y: b }"],
    );
}

#[test]
fn pat_enum() {
    check("None", |c| c.parse::<Pat>(), expect!["None"]);
    check("Some(x)", |c| c.parse::<Pat>(), expect!["Some(x)"]);
}

#[test]
fn pat_range() {
    check("1..=5", |c| c.parse::<Pat>(), expect!["1..5"]);
    check("1..", |c| c.parse::<Pat>(), expect!["1.."]);
    check("..=5", |c| c.parse::<Pat>(), expect!["..5"]);
    check("..", |c| c.parse::<Pat>(), expect![".."]);
}

#[test]
fn pat_or() {
    check("1 | 2 | 3", |c| c.parse::<Pat>(), expect!["1 | 2 | 3"]);
    check("| 1 | 2", |c| c.parse::<Pat>(), expect!["1 | 2"]);
}

#[test]
fn stmt_expr() {
    check("1;", |c| c.parse::<Stmt>(), expect!["1;"]);
    check("1", |c| c.parse::<Stmt>(), expect!["1"]);
    check("x + y;", |c| c.parse::<Stmt>(), expect!["(x + y);"]);
}

#[test]
fn stmt_let() {
    check("let x = 1;", |c| c.parse::<Stmt>(), expect!["let x = 1;"]);
    check(
        "let x: i32 = 1;",
        |c| c.parse::<Stmt>(),
        expect!["let x: i32 = 1;"],
    );
    check("let x: i32;", |c| c.parse::<Stmt>(), expect!["let x: i32;"]);
    check(
        "let mut x = 1;",
        |c| c.parse::<Stmt>(),
        expect!["let mut x = 1;"],
    );
    check(
        "let Point { x, y } = p;",
        |c| c.parse::<Stmt>(),
        expect!["let Point { x, y } = p;"],
    );
}

#[test]
fn stmt_defer() {
    check(
        "defer cleanup();",
        |c| c.parse::<Stmt>(),
        expect!["defer cleanup();"],
    );
}

#[test]
fn item_fn_simple() {
    check_file("fn main() {}", expect!["fn main() { ... }"]);
}

#[test]
fn item_fn_params() {
    check_file(
        "fn add(a: i32, b: i32) -> i32 { a }",
        expect!["fn add(a: i32, b: i32) -> i32 { ... }"],
    );
}

#[test]
fn item_fn_generic() {
    check_file(
        "fn id<T>(x: T) -> T { x }",
        expect!["fn id<T>(x: T) -> T { ... }"],
    );
}

#[test]
fn item_fn_ptr_params() {
    check_file(
        "fn f(x: *i32) -> *mut i32 { x }",
        expect!["fn f(x: *i32) -> *mut i32 { ... }"],
    );
}

#[test]
fn item_struct_record() {
    check_file(
        "struct Point { x: i32, y: i32 }",
        expect!["struct Point { x: i32, y: i32 }"],
    );
}

#[test]
fn item_struct_tuple() {
    check_file("struct Pair(i32, i32);", expect!["struct Pair(i32, i32);"]);
}

#[test]
fn item_struct_unit() {
    check_file("struct Marker;", expect!["struct Marker;"]);
}

#[test]
fn item_struct_empty() {
    check_file("struct Empty {}", expect!["struct Empty {  }"]);
}

#[test]
fn item_enum_unit_variants() {
    check_file(
        "enum Color { Red, Green, Blue }",
        expect!["enum Color { Red, Green, Blue }"],
    );
}

#[test]
fn item_enum_mixed_variants() {
    check_file(
        "enum Shape { Circle(f64), Empty }",
        expect!["enum Shape { Circle(f64), Empty }"],
    );
}

#[test]
fn item_enum_multi_field_variant() {
    check_file("enum E { A(i32, u8) }", expect!["enum E { A(i32, u8) }"]);
}

#[test]
fn item_const() {
    check_file("const MAX: i32 = 100;", expect!["const MAX: i32 = 100"]);
}

#[test]
fn item_static() {
    check_file("static X: i32 = 1;", expect!["static X: i32 = 1"]);
}

#[test]
fn item_type_alias() {
    check_file("type MyInt = i32;", expect!["type MyInt = i32"]);
}

#[test]
fn item_impl_inherent() {
    check_file(
        "impl Point { fn new() -> i32 { 0 } }",
        expect!["impl Point"],
    );
}

#[test]
fn item_impl_trait() {
    check_file(
        "impl Display for Point { }",
        expect!["impl Display for Point"],
    );
}

#[test]
fn item_mod_inline() {
    check_file("mod foo { fn bar() {} }", expect!["mod foo {...}"]);
}

#[test]
fn item_mod_external() {
    check_file("mod foo;", expect!["mod foo;"]);
}

#[test]
fn item_use_simple() {
    check_file("use foo::bar;", expect!["use foo::bar"]);
}

#[test]
fn item_use_nested() {
    check_file("use foo::{bar, baz};", expect!["use foo::{bar, baz}"]);
}

#[test]
fn item_use_glob() {
    check_file("use foo::*;", expect!["use foo::*"]);
}

#[test]
fn item_use_rename() {
    check_file("use foo::bar as baz;", expect!["use foo::bar as baz"]);
}

#[test]
fn recover_bad_fn() {
    check_file(
        "fn good1() {}\nfn bad( { }\nfn good2() {}",
        expect![[r#"
            // 1 diagnostics
            fn good1() { ... }
            fn good2() { ... }"#]],
    );
}

#[test]
fn recover_missing_item_keyword() {
    check_file(
        "fn good() {}\n???\nfn also_good() {}",
        expect![[r#"
            // 1 diagnostics
            fn good() { ... }
            fn also_good() { ... }"#]],
    );
}

#[test]
fn recover_multiple_bad() {
    check_file(
        "fn a() {}\nfn bad1( { }\nfn bad2) {\nfn c() {}",
        expect![[r#"
            // 2 diagnostics
            fn a() { ... }
            fn c() { ... }"#]],
    );
}

#[test]
fn end_to_end_simple_main() {
    check_file(
        "fn main() {\n    let x = 1;\n    let y = 2;\n    x + y;\n}",
        expect!["fn main() { ... }"],
    );
}

#[test]
fn end_to_end_struct_impl() {
    check_file(
        r#"
struct Point { x: i32, y: i32 }

impl Point {
    fn new(x: i32, y: i32) -> i32 {
        Point { x, y }
    }
}

fn main() {
    let p = Point::new(1, 2);
    let Point { x, y } = p;
}
"#,
        expect![[r#"
            struct Point { x: i32, y: i32 }
            impl Point
            fn main() { ... }"#]],
    );
}

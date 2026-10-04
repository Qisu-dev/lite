//! AST 的 Display 实现 —— 紧凑的规范形式。
//!
//! 规则：`{}` 输出规范化的源码形式（单行）。
//! 详细结构用 `{:#?}`（derive 的 Debug）。
//!
//! 所有 `impl Display` 集中于此 —— `ast.rs` 保持纯数据。

use std::fmt::{self, Display, Formatter, Write};

use traversable::{Traversable, TraversableMut};

use crate::ast::*;

/// 分隔遍历 —— `a, b, c`
fn sep<T: Display>(f: &mut Formatter<'_>, delim: &str, items: &[T]) -> fmt::Result {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            f.write_str(delim)?;
        }
        write!(f, "{}", item)?;
    }
    Ok(())
}

fn opt<T: Display>(f: &mut Formatter<'_>, v: &Option<T>) -> fmt::Result {
    if let Some(v) = v {
        write!(f, "{}", v)?;
    }
    Ok(())
}

impl Display for Ident {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.text.as_str())
    }
}

impl Display for StrLit {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // 原始文本不含引号（Lexer 已剥离），这里补回
        write!(f, "\"{}\"", self.text.as_str())
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(q) = &self.qself {
            write!(f, "{}", q)?;
        }
        sep(f, "::", &self.segments)
    }
}

impl Display for QSelf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<{} as {}>::", self.ty, self.trait_)
    }
}

impl Display for PathSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        opt(f, &self.generic_args)
    }
}

impl Display for GenericArgs {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_char('<')?;
        sep(f, ", ", &self.args)?;
        f.write_char('>')
    }
}

impl Display for GenericArg {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.kind {
            GenericArgKind::Type(t) => write!(f, "{}", t),
            GenericArgKind::Const(e) => write!(f, "{}", e),
        }
    }
}

impl Display for Generics {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.params.is_empty() {
            return Ok(());
        }
        f.write_char('<')?;
        sep(f, ", ", &self.params)?;
        f.write_char('>')
    }
}

impl Display for Generic {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(bounds) = &self.bounds {
            write!(f, ": {}", bounds)?;
        }
        Ok(())
    }
}

impl Display for Bounds {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        sep(f, " + ", &self.bounds)
    }
}

impl Display for WhereClause {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("where ")?;
        sep(f, ", ", &self.predicates)
    }
}

impl Display for WherePredicate {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            WherePredicate::TypeBound { path, bounds } => {
                write!(f, "{}: {}", path, bounds)
            }
            WherePredicate::Equality { path, value } => {
                write!(f, "{} = {}", path, value)
            }
        }
    }
}

impl Display for Mutability {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Mutability::Mut => f.write_str("mut "),
            Mutability::Immut => Ok(()),
        }
    }
}

impl Display for BinOpKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BinOpKind::Add => "+",
            BinOpKind::Sub => "-",
            BinOpKind::Mul => "*",
            BinOpKind::Div => "/",
            BinOpKind::Rem => "%",
            BinOpKind::And => "&&",
            BinOpKind::Or => "||",
            BinOpKind::BitXor => "^",
            BinOpKind::BitAnd => "&",
            BinOpKind::BitOr => "|",
            BinOpKind::Shl => "<<",
            BinOpKind::Shr => ">>",
            BinOpKind::Eq => "==",
            BinOpKind::Lt => "<",
            BinOpKind::Le => "<=",
            BinOpKind::Ne => "!=",
            BinOpKind::Ge => ">=",
            BinOpKind::Gt => ">",
        })
    }
}

impl Display for AssignOpKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AssignOpKind::AddAssign => "+=",
            AssignOpKind::SubAssign => "-=",
            AssignOpKind::MulAssign => "*=",
            AssignOpKind::DivAssign => "/=",
            AssignOpKind::RemAssign => "%=",
            AssignOpKind::BitXorAssign => "^=",
            AssignOpKind::BitAndAssign => "&=",
            AssignOpKind::BitOrAssign => "|=",
            AssignOpKind::ShlAssign => "<<=",
            AssignOpKind::ShrAssign => ">>=",
        })
    }
}

impl Display for UnOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UnOp::Not => "!",
            UnOp::BitNot => "~",
            UnOp::Neg => "-",
        })
    }
}

impl Display for RangeLimits {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RangeLimits::HalfOpen => "..",
            RangeLimits::Closed => "..=",
        })
    }
}

impl Display for Ty {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TyKind::Path(p) => write!(f, "{}", p),
            TyKind::Never => f.write_str("!"),
            TyKind::Unit => f.write_str("()"),
            TyKind::Ptr(m, inner) => {
                f.write_char('*')?;
                if *m == Mutability::Mut {
                    f.write_str("mut ")?;
                }
                write!(f, "{}", inner)
            }
            TyKind::Array { elem, len } => {
                write!(f, "[{}; {}]", elem, len)
            }
            TyKind::Slice(inner) => write!(f, "[{}]", inner),
            TyKind::Tuple(elems) => {
                f.write_char('(')?;
                sep(f, ", ", elems)?;
                f.write_char(')')
            }
            TyKind::FnPtr(params, ret) => {
                f.write_str("fn(")?;
                sep(f, ", ", params)?;
                f.write_char(')')?;
                if let Some(r) = ret {
                    write!(f, " -> {}", r)?;
                }
                Ok(())
            }
            TyKind::SelfTy => f.write_str("Self"),
            TyKind::Infer => f.write_str("_"),
            TyKind::Dyn(paths) => {
                f.write_str("dyn ")?;
                sep(f, " + ", paths)
            }
        }
    }
}

impl Display for ArrayLen {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ArrayLen::Infer => f.write_str("_"),
            ArrayLen::Expr(e) => write!(f, "{}", e),
        }
    }
}

impl Display for Pat {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PatKind::Wild => f.write_str("_"),
            PatKind::Ident(m, n) => write!(f, "{}{}", m, n),
            PatKind::Tuple(elems) => {
                f.write_char('(')?;
                sep(f, ", ", elems)?;
                f.write_char(')')
            }
            PatKind::Struct(path, fields, has_rest) => {
                write!(f, "{} {{ ", path)?;
                let mut first = true;
                for field in fields {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    write!(f, "{}", field)?;
                }
                if *has_rest {
                    if !first {
                        f.write_str(", ")?;
                    }
                    f.write_str("..")?;
                }
                f.write_str(" }")
            }
            PatKind::Enum(path, payload) => {
                write!(f, "{}", path)?;
                if let Some(p) = payload {
                    write!(f, "({})", p)?;
                }
                Ok(())
            }
            PatKind::Expr(e) => write!(f, "{}", e),
            PatKind::Range(start, end, limits) => {
                opt(f, start)?;
                write!(f, "{}", limits.value)?;
                opt(f, end)
            }
            PatKind::Rest => f.write_str(".."),
            PatKind::Slice(elems) => {
                f.write_char('[')?;
                sep(f, ", ", elems)?;
                f.write_char(']')
            }
            PatKind::Or(pats) => sep(f, " | ", pats),
        }
    }
}

impl Display for StructFieldPat {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // 简写：`x` 而不是 `x: x`
        if let PatKind::Ident(Mutability::Immut, n) = &self.pat.kind {
            if n.text == self.name.text {
                return write!(f, "{}", self.name);
            }
        }
        write!(f, "{}: {}", self.name, self.pat)
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ExprKind::Binary(l, op, r) => write!(f, "({} {} {})", l, op.value, r),
            ExprKind::Unary(op, e) => write!(f, "({}{})", op, e),
            ExprKind::Literal(lit) => f.write_str(lit.value.as_str()),
            ExprKind::Grouped(e) => write!(f, "({})", e),
            ExprKind::Assign(l, r) => write!(f, "({} = {})", l, r),
            ExprKind::AssignOp(l, op, r) => write!(f, "({} {} {})", l, op.value, r),
            ExprKind::Call(callee, args) => {
                write!(f, "{}(", callee)?;
                sep(f, ", ", args)?;
                f.write_char(')')
            }
            ExprKind::Block(b) => write!(f, "{}", b),
            ExprKind::If(cond, then, else_) => {
                write!(f, "if {} {}", cond, then)?;
                if let Some(e) = else_ {
                    write!(f, " else {}", e)?;
                }
                Ok(())
            }
            ExprKind::While(cond, body) => write!(f, "while {} {}", cond, body),
            ExprKind::For {
                variable,
                iter,
                body,
            } => write!(f, "for {} in {} {}", variable, iter, body),
            ExprKind::Index(base, idx) => write!(f, "{}[{}]", base, idx),
            ExprKind::Range(start, end, limits) => {
                opt(f, start)?;
                write!(f, "{}", limits)?;
                opt(f, end)
            }
            ExprKind::Loop(body) => write!(f, "loop {}", body),
            ExprKind::Field(base, name) => write!(f, "{}.{}", base, name),
            ExprKind::Path(p) => write!(f, "{}", p),
            ExprKind::Bool(b) => write!(f, "{}", b),
            ExprKind::Tuple(elems) => {
                f.write_char('(')?;
                sep(f, ", ", elems)?;
                f.write_char(')')
            }
            ExprKind::Unit => f.write_str("()"),
            ExprKind::AddressOf(m, e) => write!(f, "&{}{}", m, e),
            ExprKind::Deref(e) => write!(f, "*{}", e),
            ExprKind::StructExpr(s) => write!(f, "{}", s),
            ExprKind::Cast(e, ty) => write!(f, "({} as {})", e, ty),
            ExprKind::Match(scrutinee, arms) => {
                write!(f, "match {} {{ ", scrutinee)?;
                sep(f, ", ", arms)?;
                f.write_str(" }")
            }
            ExprKind::Return(Some(e)) => write!(f, "return {}", e),
            ExprKind::Return(None) => f.write_str("return"),
            ExprKind::Continue => f.write_str("continue"),
            ExprKind::Break(Some(e)) => write!(f, "break {}", e),
            ExprKind::Break(None) => f.write_str("break"),
            ExprKind::Undefined => f.write_str("undefined"),
            ExprKind::MatchBool { pattern, matched } => {
                write!(f, "let {} = {}", pattern, matched)
            }
            ExprKind::Underscore => f.write_str("_"),
            ExprKind::Closure(params, body) => {
                f.write_char('|')?;
                sep(f, ", ", params)?;
                write!(f, "| {}", body)
            }
            ExprKind::Try(e) => write!(f, "{}?", e),
            ExprKind::Array(elems) => {
                f.write_char('[')?;
                sep(f, ", ", elems)?;
                f.write_char(']')
            }
            ExprKind::ArrayRepeat(value, count) => write!(f, "[{}; {}]", value, count),
            ExprKind::Let(pat, expr) => write!(f, "let {} = {}", pat, expr),
        }
    }
}

impl Display for ClosureParam {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(ty) = &self.ty {
            write!(f, ": {}", ty)?;
        }
        Ok(())
    }
}

impl Display for StructExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} {{ ", self.path)?;
        sep(f, ", ", &self.fields)?;
        f.write_str(" }")
    }
}

impl Display for StructExprField {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.is_shorthand {
            write!(f, "{}", self.name)
        } else {
            write!(f, "{}: {}", self.name, self.value)
        }
    }
}

impl Display for Stmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.kind {
            StmtKind::Expr(e) => write!(f, "{}", e),
            StmtKind::Semi(e) => write!(f, "{};", e),
            StmtKind::Let(pat, ty, init) => {
                write!(f, "let {}", pat)?;
                if let Some(ty) = ty {
                    write!(f, ": {}", ty)?;
                }
                if let Some(init) = init {
                    write!(f, " = {}", init)?;
                }
                f.write_char(';')
            }
            StmtKind::Defer(e) => write!(f, "defer {};", e),
            StmtKind::Item(i) => write!(f, "{}", i),
        }
    }
}

impl Display for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // 紧凑形式：{ stmt1; stmt2 }
        f.write_str("{ ")?;
        sep(f, " ", &self.stmts)?;
        f.write_str(" }")
    }
}

impl Display for Arm {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pat)?;
        if let Some(guard) = &self.guard {
            write!(f, " if {}", guard)?;
        }
        write!(f, " => {}", self.body)
    }
}

impl<K: Display + Traversable + TraversableMut> Display for Item<K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for attr in &self.attr {
            write!(f, "{} ", attr)?;
        }
        if self.visibility == Visibility::Public {
            f.write_str("pub ")?;
        }
        write!(f, "{}", self.kind)
    }
}

impl Display for ItemKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ItemKind::Fn(func) => write!(f, "{}", func),
            ItemKind::Struct(s) => write!(f, "{}", s),
            ItemKind::Union(u) => write!(f, "{}", u),
            ItemKind::Enum(name, generics, variants) => {
                write!(f, "enum {}{} {{ ", name, generics)?;
                sep(f, ", ", variants)?;
                f.write_str(" }")
            }
            ItemKind::Trait(name, generics, items) => {
                write!(f, "trait {}{} {{ {} items }}", name, generics, items.len())
            }
            ItemKind::Impl(imp) => write!(f, "{}", imp),
            ItemKind::Use(tree) => write!(f, "use {};", tree),
            ItemKind::Module(name, inline) => match inline {
                Inline::Inline(_) => write!(f, "mod {} {{...}}", name),
                Inline::External(_) => write!(f, "mod {};", name),
            },
            ItemKind::Const(name, ty, init) => {
                write!(f, "const {}: {} = {};", name, ty, init)
            }
            ItemKind::Static(m, name, ty, init) => {
                write!(f, "static {}{}: {} = {};", m, name, ty, init)
            }
            ItemKind::TypeAlias(ta) => write!(f, "{}", ta),
            ItemKind::Extern(ext) => write!(f, "{}", ext),
        }
    }
}

impl Display for Fn {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.sig)?;
        if self.body.is_some() {
            f.write_str(" { ... }")
        } else {
            f.write_char(';')
        }
    }
}

impl Display for FnSig {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "fn {}{}(", self.name, self.generics)?;
        sep(f, ", ", &self.params)?;
        if self.is_variadic {
            if !self.params.is_empty() {
                f.write_str(", ")?;
            }
            f.write_str("...")?;
        }
        f.write_char(')')?;
        match &self.return_type {
            FnRetTy::Default(_) => Ok(()),
            FnRetTy::Ty(ty) => write!(f, " -> {}", ty),
        }
    }
}

impl Display for Param {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParamKind::Normal(pat, ty) => write!(f, "{}: {}", pat, ty),
            ParamKind::SelfValue(m) => write!(f, "{}self", m),
            ParamKind::SelfPtr(m) => {
                f.write_char('*')?;
                if *m == Mutability::Mut {
                    f.write_str("mut ")?;
                }
                f.write_str("self")
            }
        }
    }
}

impl Display for StructData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "struct {}{}", self.name, self.generics)?;
        write_variant_data(f, &self.kind)
    }
}

impl Display for UnionData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "union {}{}", self.name, self.generics)?;
        write_variant_data(f, &self.kind)
    }
}

fn write_variant_data(f: &mut Formatter<'_>, v: &VariantData) -> fmt::Result {
    match v {
        VariantData::Unit => f.write_char(';'),
        VariantData::Tuple(tys) => {
            f.write_char('(')?;
            sep(f, ", ", tys)?;
            f.write_str(");")
        }
        VariantData::Struct(fields) => {
            f.write_str(" { ")?;
            sep(f, ", ", fields)?;
            f.write_str(" }")
        }
    }
}

impl Display for Variant {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ident)?;
        match &self.data {
            VariantData::Unit => Ok(()),
            VariantData::Tuple(tys) => {
                f.write_char('(')?;
                sep(f, ", ", tys)?;
                f.write_char(')')
            }
            VariantData::Struct(fields) => {
                f.write_str(" { ")?;
                sep(f, ", ", fields)?;
                f.write_str(" }")
            }
        }
    }
}

impl Display for FieldData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.visibility == Visibility::Public {
            f.write_str("pub ")?;
        }
        write!(f, "{}: {}", self.name, self.ty)
    }
}

impl Display for Field {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.visibility == Visibility::Public {
            f.write_str("pub ")?;
        }
        write!(f, "{}: {}", self.name, self.ty)
    }
}

impl Display for TypeAlias {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "type {}{} = {};", self.name, self.generics, self.ty)
    }
}

impl Display for AssociatedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "type {}", self.name)?;
        if let Some(b) = &self.bounds {
            write!(f, ": {}", b)?;
        }
        if let Some(d) = &self.default_ty {
            write!(f, " = {}", d)?;
        }
        f.write_char(';')
    }
}

impl Display for AssociatedConstant {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "const {}: {}", self.name, self.ty)?;
        if let Some(e) = &self.default_expr {
            write!(f, " = {}", e)?;
        }
        f.write_char(';')
    }
}

impl Display for Impl {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "impl{} ", self.generics)?;
        if let Some(t) = &self.of_trait {
            write!(f, "{} for ", t)?;
        }
        write!(f, "{} {{ {} items }}", self.self_ty, self.items.len())
    }
}

impl Display for ImplItemKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ImplItemKind::Fn(func) => write!(f, "{}", func),
            ImplItemKind::Ty(ta) => write!(f, "{}", ta),
            ImplItemKind::Const(c) => write!(f, "{}", c),
        }
    }
}

impl Display for TraitItemKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TraitItemKind::Fn(sig) => write!(f, "{};", sig),
            TraitItemKind::Ty(ta) => write!(f, "{}", ta),
        }
    }
}

impl Display for Extern {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("extern ")?;
        if let Some(abi) = &self.abi {
            write!(f, "\"{}\" ", abi)?;
        }
        write!(f, "{{ {} items }}", self.items.len())
    }
}

impl Display for ExternItemKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ExternItemKind::Fn(func) => write!(f, "{}", func),
            ExternItemKind::Struct(s) => write!(f, "{}", s),
            ExternItemKind::Enum(e) => {
                write!(f, "enum {} {{ {} variants }}", e.name, e.kind.len())
            }
            ExternItemKind::Union(u) => write!(f, "{}", u),
        }
    }
}

impl Display for UseTree {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.prefix)?;
        match &self.kind {
            UseTreeKind::Simple(None) => Ok(()),
            UseTreeKind::Simple(Some(rename)) => write!(f, " as {}", rename),
            UseTreeKind::Glob => f.write_str("::*"),
            UseTreeKind::Nested(trees, _) => {
                f.write_str("::{")?;
                sep(f, ", ", trees)?;
                f.write_char('}')
            }
        }
    }
}

impl Display for Attr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.path)?;
        if !self.args.is_empty() {
            f.write_char('(')?;
            sep(f, ", ", &self.args)?;
            f.write_char(')')?;
        }
        Ok(())
    }
}

impl Display for AttrArg {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            AttrArg::Path(p) => write!(f, "{}", p),
            AttrArg::Lit(l) => write!(f, "{}", l.value.as_str()),
            AttrArg::KeyValue { key, value } => write!(f, "{} = {}", key, value),
        }
    }
}

impl Display for Crate {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                f.write_char('\n')?;
            }
            write!(f, "{}", item)?;
        }
        Ok(())
    }
}

use crate::parser::recovery::SYNC_ITEM;
use crate::parser::{Expected, ParseCtx, recovery::Recovery, trait_::Parse};
use litec_ast::{
    ast::{
        AssociatedConstant, AssociatedType, Attr, AttrArg, Block, Bounds, Enum, Extern, ExternItem,
        ExternItemKind, FieldData, Fn, FnRetTy, FnSig, Generic, Generics, Ident, Impl, ImplItem,
        ImplItemKind, Inline, Item, ItemKind, Mutability, Param, ParamKind, Pat, Path, StructData,
        TraitItem, TraitItemKind, Ty, TyKind, TypeAlias, UnionData, UseTree, UseTreeKind, Variant,
        VariantData, Visibility, WhereClause, WherePredicate,
    },
    tok,
};
use litec_error::{Diag, PResult};
use litec_span::Span;

impl Parse for Item {
    fn recovery() -> Recovery {
        Recovery::SkipTo(SYNC_ITEM)
    }

    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        ctx.parse_item()
    }
}

impl Parse for TraitItem {
    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        ctx.parse_trait_item()
    }
}

impl Parse for ImplItem {
    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        ctx.parse_impl_item()
    }
}

impl Parse for ExternItem {
    fn parse(ctx: &mut ParseCtx<'_, '_>) -> PResult<Self> {
        ctx.parse_extern_item()
    }
}

impl<'a, 'src> ParseCtx<'a, 'src> {
    pub(crate) fn parse_item(&mut self) -> PResult<Item> {
        let start = self.current_span();
        let attrs = self.parse_attrs()?;
        let vis = self.parse_visibility();

        let kind = match self.peek_kind() {
            tok!(fn) => ItemKind::Fn(self.parse_fn()?),
            tok!(struct) => ItemKind::Struct(self.parse_struct()?),
            tok!(enum) => {
                let e = self.parse_enum()?;
                ItemKind::Enum(e.name, e.generics, e.kind)
            }
            tok!(union) => ItemKind::Union(self.parse_union()?),
            tok!(trait) => {
                let (name, generics, items) = self.parse_trait()?;
                ItemKind::Trait(name, generics, items)
            }
            tok!(impl) => ItemKind::Impl(self.parse_impl()?),
            tok!(mod) => self.parse_module()?,
            tok!(use) => ItemKind::Use(self.parse_use_tree()?),
            tok!(const) => {
                let (name, ty, init) = self.parse_const()?;
                ItemKind::Const(name, ty, init)
            }
            tok!(static) => {
                let (mutability, name, ty, init) = self.parse_static()?;
                ItemKind::Static(mutability, name, ty, init)
            }
            tok!(type) => ItemKind::TypeAlias(self.parse_type_alias()?),
            tok!(extern) => ItemKind::Extern(self.parse_extern()?),
            _ => {
                return Err(self.unexpected(&[
                    Expected::Exact(tok!(fn)),
                    Expected::Exact(tok!(struct)),
                    Expected::Exact(tok!(enum)),
                    Expected::Exact(tok!(trait)),
                    Expected::Exact(tok!(impl)),
                    Expected::Exact(tok!(mod)),
                    Expected::Exact(tok!(use)),
                    Expected::Exact(tok!(const)),
                    Expected::Exact(tok!(static)),
                    Expected::Exact(tok!(type)),
                    Expected::Exact(tok!(extern)),
                ]));
            }
        };

        Ok(Item {
            node_id: self.node_id(),
            attr: attrs,
            visibility: vis,
            span: start.extend(self.prev_span()),
            kind,
        })
    }

    fn parse_attrs(&mut self) -> PResult<Vec<Attr>> {
        let mut attrs = Vec::new();
        while self.check(tok!(@)) {
            attrs.push(self.parse_attr()?);
        }
        Ok(attrs)
    }

    fn parse_attr(&mut self) -> PResult<Attr> {
        let start = self.expect(tok!(@))?.span;
        let path: Path = self.parse()?;

        let mut args = Vec::new();
        if self.eat(tok!(OpenParen)) {
            while !self.check(tok!(CloseParen)) && !self.at_eof() {
                args.push(self.parse_attr_arg()?);
                if !self.eat(tok!(,)) {
                    break;
                }
            }
            self.expect(tok!(CloseParen))?;
        }

        let span = start.extend(self.prev_span());
        Ok(Attr { path, args, span })
    }

    fn parse_attr_arg(&mut self) -> PResult<AttrArg> {
        // 键值 `key = value`
        if self.check(tok!(Ident)) && self.peek(1).kind == tok!(=) {
            let key: Ident = self.parse()?;
            self.expect(tok!(=))?;
            let value = self.parse_attr_arg()?;
            return Ok(AttrArg::KeyValue {
                key,
                value: Box::new(value),
            });
        }

        if self.peek_kind().is_literal() {
            let lit = self.parse()?;
            return Ok(AttrArg::Lit(lit));
        }

        let path: Path = self.parse()?;
        Ok(AttrArg::Path(path))
    }

    fn parse_visibility(&mut self) -> Visibility {
        if self.eat(tok!(pub)) {
            Visibility::Public
        } else {
            Visibility::Inherited
        }
    }

    pub(crate) fn parse_fn(&mut self) -> PResult<Fn> {
        let sig = self.parse_fn_sig()?;
        let body = if self.check(tok!(OpenBrace)) {
            Some(self.parse::<Block>()?)
        } else {
            self.eat(tok!(;));
            None
        };
        Ok(Fn {
            node_id: self.node_id(),
            sig,
            body,
        })
    }

    pub(crate) fn parse_fn_sig(&mut self) -> PResult<FnSig> {
        self.expect(tok!(fn))?;
        let name: Ident = self.parse()?;
        let generics = self.parse_generics()?;

        self.expect(tok!(OpenParen))?;
        let mut params = Vec::new();
        let mut is_variadic = false;
        while !self.check(tok!(CloseParen)) && !self.at_eof() {
            if self.eat(tok!(...)) {
                is_variadic = true;
                break;
            }
            params.push(self.parse_param()?);
            if !self.eat(tok!(,)) {
                break;
            }
        }
        self.expect(tok!(CloseParen))?;

        let return_type = if self.eat(tok!(->)) {
            FnRetTy::Ty(self.parse()?)
        } else {
            FnRetTy::Default(self.prev_span())
        };

        Ok(FnSig {
            name,
            generics,
            params,
            return_type,
            is_variadic,
            abi: None,
        })
    }

    fn parse_param(&mut self) -> PResult<Param> {
        let start = self.current_span();

        // `self` / `mut self`
        if self.check(tok!(self)) {
            let s = self.bump().span;
            return Ok(Param {
                node_id: self.node_id(),
                kind: ParamKind::SelfValue(Mutability::Immut),
                span: s,
            });
        }
        if self.check(tok!(mut)) && self.peek(1).kind == tok!(self) {
            let s = self.bump().span;
            let end = self.bump().span;
            return Ok(Param {
                node_id: self.node_id(),
                kind: ParamKind::SelfValue(Mutability::Mut),
                span: s.extend(end),
            });
        }
        // `*self` / `*mut self`
        if self.check(tok!(*)) && self.peek(1).kind == tok!(self) {
            let s = self.bump().span;
            let end = self.bump().span;
            return Ok(Param {
                node_id: self.node_id(),
                kind: ParamKind::SelfPtr(Mutability::Immut),
                span: s.extend(end),
            });
        }
        if self.check(tok!(*)) && self.peek(1).kind == tok!(mut) && self.peek(2).kind == tok!(self)
        {
            let s = self.bump().span;
            self.bump();
            let end = self.bump().span;
            return Ok(Param {
                node_id: self.node_id(),
                kind: ParamKind::SelfPtr(Mutability::Mut),
                span: s.extend(end),
            });
        }

        // 普通参数
        let pat: Pat = self.parse()?;
        self.expect(tok!(:))?;
        let ty: Ty = self.parse()?;

        let span = start.extend(ty.span);
        Ok(Param {
            node_id: self.node_id(),
            kind: ParamKind::Normal(Box::new(pat), Box::new(ty)),
            span,
        })
    }

    pub(crate) fn parse_generics(&mut self) -> PResult<Generics> {
        let start = self.current_span();
        let mut params = Vec::new();

        if !self.eat(tok!(<)) {
            return Ok(Generics::empty());
        }

        while !self.check(tok!(>)) && !self.at_eof() {
            self.split_shr_to_gt();
            let param_start = self.current_span();
            let name: Ident = self.parse()?;
            let bounds = if self.eat(tok!(:)) {
                Some(self.parse_bounds()?)
            } else {
                None
            };
            params.push(Generic {
                node_id: self.node_id(),
                name,
                bounds,
                span: param_start.extend(self.prev_span()),
            });
            if !self.eat(tok!(,)) {
                break;
            }
        }
        self.split_shr_to_gt();
        let end = self.expect(tok!(>))?.span;

        Ok(Generics {
            node_id: self.node_id(),
            params,
            span: start.extend(end),
        })
    }

    pub(crate) fn parse_bounds(&mut self) -> PResult<Bounds> {
        let start = self.current_span();
        let mut bounds = vec![self.parse::<Path>()?];
        while self.eat(tok!(+)) {
            bounds.push(self.parse()?);
        }
        Ok(Bounds {
            node_id: self.node_id(),
            bounds,
            span: start.extend(self.prev_span()),
        })
    }

    pub(crate) fn parse_where_clause(&mut self) -> PResult<Option<WhereClause>> {
        if !self.eat(tok!(where)) {
            return Ok(None);
        }
        let start = self.prev_span();
        let mut predicates = Vec::new();

        loop {
            if self.check(tok!(OpenBrace)) || self.check(tok!(;)) || self.at_eof() {
                break;
            }

            let path: Path = self.parse()?;
            if self.eat(tok!(=)) {
                let value = self.parse()?;
                predicates.push(WherePredicate::Equality { path, value });
            } else {
                self.expect(tok!(:))?;
                let bounds = self.parse_bounds()?;
                predicates.push(WherePredicate::TypeBound { path, bounds });
            }

            if !self.eat(tok!(,)) {
                break;
            }
        }

        Ok(Some(WhereClause {
            node_id: self.node_id(),
            predicates,
            span: start.extend(self.prev_span()),
        }))
    }

    pub(crate) fn parse_struct(&mut self) -> PResult<StructData> {
        self.expect(tok!(struct))?;
        let name: Ident = self.parse()?;
        let generics = self.parse_generics()?;
        let where_clause = self.parse_where_clause()?;
        let kind = self.parse_variant_data(false)?;

        Ok(StructData {
            node_id: self.node_id(),
            name,
            generics,
            where_clause,
            kind,
        })
    }

    pub(crate) fn parse_union(&mut self) -> PResult<UnionData> {
        self.expect(tok!(union))?;
        let name: Ident = self.parse()?;
        let generics = self.parse_generics()?;
        let where_clause = self.parse_where_clause()?;
        let kind = self.parse_variant_data(true)?;
        Ok(UnionData {
            node_id: self.node_id(),
            name,
            generics,
            where_clause,
            kind,
        })
    }

    pub(crate) fn parse_enum(&mut self) -> PResult<Enum> {
        self.expect(tok!(enum))?;
        let name: Ident = self.parse()?;
        let generics = self.parse_generics()?;
        let where_clause = self.parse_where_clause()?;

        self.expect(tok!(OpenBrace))?;
        let mut variants = Vec::new();
        while !self.check(tok!(CloseBrace)) && !self.at_eof() {
            variants.push(self.parse_variant()?);
            if !self.eat(tok!(,)) {
                break;
            }
        }
        self.expect(tok!(CloseBrace))?;

        Ok(Enum {
            node_id: self.node_id(),
            name,
            generics,
            where_clause,
            kind: variants,
        })
    }

    fn parse_variant(&mut self) -> PResult<Variant> {
        let start = self.current_span();
        let ident: Ident = self.parse()?;
        let data = self.parse_variant_data(false)?;
        Ok(Variant {
            node_id: self.node_id(),
            ident,
            data,
            span: start.extend(self.prev_span()),
        })
    }

    /// `A` / `A(T1, T2)` / `A { x: T }`
    fn parse_variant_data(&mut self, struct_only_named: bool) -> PResult<VariantData> {
        match self.peek_kind() {
            tok!(OpenBrace) => {
                self.bump();
                let mut fields = Vec::new();
                while !self.check(tok!(CloseBrace)) && !self.at_eof() {
                    fields.push(self.parse_field_data()?);
                    if !self.eat(tok!(,)) {
                        break;
                    }
                }
                self.expect(tok!(CloseBrace))?;
                Ok(VariantData::Struct(fields))
            }
            tok!(OpenParen) if !struct_only_named => {
                self.bump();
                let mut tys = Vec::new();
                while !self.check(tok!(CloseParen)) && !self.at_eof() {
                    tys.push(self.parse()?);
                    if !self.eat(tok!(,)) {
                        break;
                    }
                }
                self.expect(tok!(CloseParen))?;
                self.eat(tok!(;));
                Ok(VariantData::Tuple(tys))
            }
            _ => {
                self.eat(tok!(;));
                Ok(VariantData::Unit)
            }
        }
    }

    fn parse_field_data(&mut self) -> PResult<FieldData> {
        let start = self.current_span();
        let visibility = self.parse_visibility();
        let name: Ident = self.parse()?;
        self.expect(tok!(:))?;
        let ty: Ty = self.parse()?;
        Ok(FieldData {
            node_id: self.node_id(),
            visibility,
            name,
            ty,
            span: start.extend(self.prev_span()),
        })
    }

    /// 返回 `(name, generics, items)`——匹配 `ItemKind::Trait` 的字段
    fn parse_trait(&mut self) -> PResult<(Ident, Generics, Vec<TraitItem>)> {
        self.expect(tok!(trait))?;
        let name: Ident = self.parse()?;
        let generics = self.parse_generics()?;

        // super traits
        if self.eat(tok!(:)) {
            let _ = self.parse_bounds()?;
        }

        let _ = self.parse_where_clause()?;

        self.expect(tok!(OpenBrace))?;
        let mut items = Vec::new();
        while !self.check(tok!(CloseBrace)) && !self.at_eof() {
            items.push(self.parse_trait_item()?);
        }
        self.expect(tok!(CloseBrace))?;

        Ok((name, generics, items))
    }

    pub(crate) fn parse_trait_item(&mut self) -> PResult<TraitItem> {
        let start = self.current_span();
        let attrs = self.parse_attrs()?;
        let vis = self.parse_visibility();

        let kind = match self.peek_kind() {
            tok!(fn) => TraitItemKind::Fn(self.parse_fn_sig()?),
            tok!(type) => {
                self.bump();
                TraitItemKind::Ty(self.parse_assoc_type_body()?)
            }
            _ => {
                return Err(
                    self.unexpected(&[Expected::Exact(tok!(fn)), Expected::Exact(tok!(type))])
                );
            }
        };

        Ok(Item {
            node_id: self.node_id(),
            attr: attrs,
            visibility: vis,
            span: start.extend(self.prev_span()),
            kind,
        })
    }

    /// 解析 `type Name: Bounds = Default;` 的 body 部分（`type` 关键字已消费）
    fn parse_assoc_type_body(&mut self) -> PResult<TypeAlias> {
        let name: Ident = self.parse()?;
        let generics = self.parse_generics()?;
        let _bounds = if self.eat(tok!(:)) {
            Some(self.parse_bounds()?)
        } else {
            None
        };
        let where_clause = self.parse_where_clause()?;
        let ty = if self.eat(tok!(=)) {
            self.parse()?
        } else {
            Ty {
                node_id: self.node_id(),
                kind: TyKind::Infer,
                span: self.current_span(),
            }
        };
        self.eat(tok!(;));

        Ok(TypeAlias {
            node_id: self.node_id(),
            name,
            generics,
            where_clause,
            ty,
        })
    }

    fn parse_impl(&mut self) -> PResult<Impl> {
        self.expect(tok!(impl))?;
        let generics = self.parse_generics()?;

        let first: Ty = self.parse()?;

        let (of_trait, self_ty) = if self.eat(tok!(for)) {
            let trait_path = match first.kind {
                TyKind::Path(p) => p,
                _ => {
                    return Err(self.emit(Diag::error("expected trait path").with_span(first.span)));
                }
            };
            let self_ty: Ty = self.parse()?;
            (Some(trait_path), Box::new(self_ty))
        } else {
            (None, Box::new(first))
        };

        let _ = self.parse_where_clause()?;
        self.expect(tok!(OpenBrace))?;

        let mut items = Vec::new();
        while !self.check(tok!(CloseBrace)) && !self.at_eof() {
            items.push(self.parse_impl_item()?);
        }
        self.expect(tok!(CloseBrace))?;

        Ok(Impl {
            node_id: self.node_id(),
            generics,
            of_trait,
            self_ty,
            items,
        })
    }

    pub(crate) fn parse_impl_item(&mut self) -> PResult<ImplItem> {
        let start = self.current_span();
        let attrs = self.parse_attrs()?;
        let vis = self.parse_visibility();

        let kind = match self.peek_kind() {
            tok!(fn) => ImplItemKind::Fn(self.parse_fn()?),
            tok!(type) => {
                self.bump();
                let name: Ident = self.parse()?;
                let generics = self.parse_generics()?;
                let bounds = if self.eat(tok!(:)) {
                    Some(self.parse_bounds()?)
                } else {
                    None
                };
                let where_clause = self.parse_where_clause()?;
                let default_ty = if self.eat(tok!(=)) {
                    Some(self.parse::<Ty>()?)
                } else {
                    None
                };
                self.eat(tok!(;));
                ImplItemKind::Ty(AssociatedType {
                    node_id: self.node_id(),
                    name,
                    bounds,
                    where_clause,
                    default_ty,
                    span: start.extend(self.prev_span()),
                })
            }
            tok!(const) => {
                self.bump();
                let name: Ident = self.parse()?;
                self.expect(tok!(:))?;
                let ty: Ty = self.parse()?;
                let default_expr = if self.eat(tok!(=)) {
                    Some(self.parse()?)
                } else {
                    None
                };
                self.eat(tok!(;));
                ImplItemKind::Const(AssociatedConstant {
                    node_id: self.node_id(),
                    name,
                    ty,
                    default_expr,
                    span: start.extend(self.prev_span()),
                })
            }
            _ => {
                return Err(self.unexpected(&[
                    Expected::Exact(tok!(fn)),
                    Expected::Exact(tok!(type)),
                    Expected::Exact(tok!(const)),
                ]));
            }
        };

        Ok(Item {
            node_id: self.node_id(),
            attr: attrs,
            visibility: vis,
            span: start.extend(self.prev_span()),
            kind,
        })
    }

    fn parse_module(&mut self) -> PResult<ItemKind> {
        self.expect(tok!(mod))?;
        let name: Ident = self.parse()?;

        if self.eat(tok!(;)) {
            return Ok(ItemKind::Module(name, Inline::External(Vec::new())));
        }

        self.expect(tok!(OpenBrace))?;
        let mut items = Vec::new();
        while !self.check(tok!(CloseBrace)) && !self.at_eof() {
            items.push(self.parse_item()?);
        }
        self.expect(tok!(CloseBrace))?;

        Ok(ItemKind::Module(name, Inline::Inline(items)))
    }

    pub(crate) fn parse_use_tree(&mut self) -> PResult<UseTree> {
        let start = self.current_span();
        self.expect(tok!(use))?;
        let tree = self.parse_use_tree_body(start)?;
        self.eat(tok!(;));
        Ok(tree)
    }

    fn parse_use_tree_body(&mut self, start: Span) -> PResult<UseTree> {
        // 前缀路径段
        let mut prefix_segments = vec![self.parse::<litec_ast::ast::PathSegment>()?];
        loop {
            if self.peek_kind() == tok!(::) && self.peek(1).kind == tok!(Ident) {
                self.bump(); // `::`
                prefix_segments.push(self.parse()?);
            } else {
                break;
            }
        }

        let prefix = Path {
            node_id: self.node_id(),
            segments: prefix_segments,
            span: start.extend(self.prev_span()),
            qself: None,
        };

        let kind = if self.eat(tok!(::)) {
            match self.peek_kind() {
                tok!(*) => {
                    self.bump();
                    UseTreeKind::Glob
                }
                tok!(OpenBrace) => {
                    let nstart = self.bump().span;
                    let mut trees = Vec::new();
                    while !self.check(tok!(CloseBrace)) && !self.at_eof() {
                        let tstart = self.current_span();
                        trees.push(self.parse_use_tree_body(tstart)?);
                        if !self.eat(tok!(,)) {
                            break;
                        }
                    }
                    let close = self.expect(tok!(CloseBrace))?.span;
                    UseTreeKind::Nested(trees, nstart.extend(close))
                }
                _ => {
                    let _seg: litec_ast::ast::PathSegment = self.parse()?;
                    let rename = if self.eat(tok!(as)) {
                        Some(self.parse()?)
                    } else {
                        None
                    };
                    UseTreeKind::Simple(rename)
                }
            }
        } else {
            let rename = if self.eat(tok!(as)) {
                Some(self.parse()?)
            } else {
                None
            };
            UseTreeKind::Simple(rename)
        };

        Ok(UseTree {
            node_id: self.node_id(),
            prefix,
            kind,
            span: start.extend(self.prev_span()),
        })
    }

    /// 返回 `(name, ty, init)`——匹配 `ItemKind::Const` 的字段
    fn parse_const(&mut self) -> PResult<(Ident, Ty, litec_ast::ast::Expr)> {
        self.expect(tok!(const))?;
        let name: Ident = self.parse()?;
        self.expect(tok!(:))?;
        let ty: Ty = self.parse()?;
        self.expect(tok!(=))?;
        let init = self.parse()?;
        self.eat(tok!(;));
        Ok((name, ty, init))
    }

    /// 返回 `(mutability, name, ty, init)`——匹配 `ItemKind::Static` 的字段
    fn parse_static(&mut self) -> PResult<(Mutability, Ident, Ty, litec_ast::ast::Expr)> {
        self.expect(tok!(static))?;
        let mutability = if self.eat(tok!(mut)) {
            Mutability::Mut
        } else {
            Mutability::Immut
        };
        let name: Ident = self.parse()?;
        self.expect(tok!(:))?;
        let ty: Ty = self.parse()?;
        self.expect(tok!(=))?;
        let init = self.parse()?;
        self.eat(tok!(;));
        Ok((mutability, name, ty, init))
    }

    fn parse_type_alias(&mut self) -> PResult<TypeAlias> {
        self.expect(tok!(type))?;
        let name: Ident = self.parse()?;
        let generics = self.parse_generics()?;
        let where_clause = self.parse_where_clause()?;
        self.expect(tok!(=))?;
        let ty: Ty = self.parse()?;
        self.eat(tok!(;));
        Ok(TypeAlias {
            node_id: self.node_id(),
            name,
            generics,
            where_clause,
            ty,
        })
    }

    fn parse_extern(&mut self) -> PResult<Extern> {
        self.expect(tok!(extern))?;
        // TODO: `extern "C"` 的 abi 是字符串字面量，但 `Extern.abi` 是 `Option<Ident>`——
        // AST 需要调整。目前跳过。
        let abi = None;

        self.expect(tok!(OpenBrace))?;
        let mut items = Vec::new();
        while !self.check(tok!(CloseBrace)) && !self.at_eof() {
            items.push(self.parse_extern_item()?);
        }
        self.expect(tok!(CloseBrace))?;

        Ok(Extern {
            node_id: self.node_id(),
            abi,
            items,
        })
    }

    pub(crate) fn parse_extern_item(&mut self) -> PResult<ExternItem> {
        let start = self.current_span();
        let attrs = self.parse_attrs()?;
        let vis = self.parse_visibility();

        let kind = match self.peek_kind() {
            tok!(fn) => ExternItemKind::Fn(self.parse_fn()?),
            tok!(struct) => ExternItemKind::Struct(self.parse_struct()?),
            tok!(enum) => {
                let e = self.parse_enum()?;
                ExternItemKind::Enum(e)
            }
            tok!(union) => ExternItemKind::Union(self.parse_union()?),
            _ => {
                return Err(self.unexpected(&[
                    Expected::Exact(tok!(fn)),
                    Expected::Exact(tok!(struct)),
                    Expected::Exact(tok!(enum)),
                    Expected::Exact(tok!(union)),
                ]));
            }
        };

        Ok(Item {
            node_id: self.node_id(),
            attr: attrs,
            visibility: vis,
            span: start.extend(self.prev_span()),
            kind,
        })
    }
}

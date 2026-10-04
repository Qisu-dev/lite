#![allow(deprecated)]

use crate::with_session_globals;
use litec_macros::symbols;
use rustc_hash::FxBuildHasher;
use rustc_hash::FxHashMap;
use stable_arena::DroplessArena;
use std::collections::hash_map::Entry;
use std::fmt::Display;
use std::sync::RwLock;
use traversable::Traversable;
use traversable::TraversableMut;

#[repr(transparent)]
#[derive(Traversable, TraversableMut, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[traverse(skip_self)]
pub struct Symbol(#[traverse(skip)] SymbolIndex);

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::fmt::Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Symbol {
    pub const fn new(n: u32) -> Self {
        Self(SymbolIndex::from_raw_unchecked(n))
    }

    pub fn intern(s: &str) -> Self {
        with_session_globals(|session| session.interner.intern(s))
    }

    /// SAFETY: 对于SessionGlobals,他的生命周期是一整个线程,相当于'static
    /// Symbol创造只能用&str所以他必定为utf8
    pub fn as_str(&self) -> &str {
        with_session_globals(|session| unsafe {
            std::mem::transmute(session.interner.get_str(*self))
        })
    }
}

#[repr(transparent)]
#[derive(Traversable, TraversableMut, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[traverse(skip_self)]
pub struct ByteSymbol(#[traverse(skip)] SymbolIndex);

impl ByteSymbol {
    pub const fn new(n: u32) -> Self {
        Self(SymbolIndex::from_raw_unchecked(n))
    }

    pub fn intern_byte_str(s: &[u8]) -> Self {
        with_session_globals(|session| session.interner.intern_byte_str(s))
    }

    /// SAFETY: Interner的生命周期等同于SessionGlobals,而SessionGlobals生命周期为'static
    pub fn as_bytes(&self) -> &'static [u8] {
        with_session_globals(|session| unsafe {
            std::mem::transmute(session.interner.get_byte_str(*self))
        })
    }
}

index_vec::define_index_type! {
   pub struct SymbolIndex = u32;
    DEBUG_FORMAT = "SymbolIndex({})";
}

struct InternerInner {
    arena: DroplessArena,
    map: FxHashMap<&'static [u8], u32>,
    byte_strs: Vec<&'static [u8]>,
}

#[repr(transparent)]
pub(crate) struct Interner(RwLock<InternerInner>);

impl Interner {
    fn prefill(init: &[&'static str], extra: &[&'static str]) -> Self {
        let values = init
            .iter()
            .copied()
            .chain(extra.iter().copied())
            .map(|s| s.as_bytes());
        let (size_hint, _) = values.size_hint();
        let mut map: FxHashMap<&[u8], u32> =
            FxHashMap::with_capacity_and_hasher(size_hint, FxBuildHasher::default());
        let mut byte_strs = Vec::with_capacity(size_hint);

        #[cfg(debug_assertions)]
        let mut conflicting_values = Vec::new();

        for v in values {
            match map.entry(v) {
                Entry::Occupied(v) => {
                    #[cfg(debug_assertions)]
                    conflicting_values.push(*v.key());
                }
                Entry::Vacant(view) => {
                    view.insert_entry(byte_strs.len() as u32);
                    byte_strs.push(v);
                }
            }
        }

        #[cfg(debug_assertions)]
        if conflicting_values.len() != 0 {
            panic!("extra与init重合: {:?}", conflicting_values);
        }

        #[cfg(debug_assertions)]
        {
            for (name, symbol) in ALL_NAMES.iter().zip(ALL_SYMBOLS) {
                let str = byte_strs
                    .get(symbol.0.index())
                    .expect("内置symbol对应的字符串不存在");

                assert!(str == &name.as_bytes(), "内置symbol与实际内容不匹配");
            }
        }

        let inner = InternerInner {
            arena: Default::default(),
            map,
            byte_strs,
        };

        Self(RwLock::new(inner))
    }

    pub fn intern(&self, s: &str) -> Symbol {
        Symbol::new(self.intern_inner(s.as_bytes()))
    }

    pub fn intern_byte_str(&self, byte_str: &[u8]) -> ByteSymbol {
        ByteSymbol::new(self.intern_inner(byte_str))
    }

    #[inline]
    fn intern_inner(&self, s: &[u8]) -> u32 {
        {
            let guard = self.0.read().unwrap();
            if let Some(&idx) = guard.map.get(s) {
                return idx;
            }
        }

        let mut guard = self.0.write().unwrap();

        if let Some(&idx) = guard.map.get(s) {
            return idx;
        }

        let byte_str: &'static [u8] = unsafe { &*(guard.arena.alloc_slice(s) as *const [u8]) };

        let idx = guard.byte_strs.len() as u32;
        guard.map.insert(byte_str, idx);
        guard.byte_strs.push(byte_str);

        idx
    }

    fn get_str(&self, symbol: Symbol) -> &str {
        let byte_str = self.0.read().unwrap().byte_strs[symbol.0.index()];
        unsafe { str::from_utf8_unchecked(byte_str) }
    }

    fn get_byte_str(&self, symbol: ByteSymbol) -> &[u8] {
        self.0.read().unwrap().byte_strs[symbol.0.index()]
    }

    pub(crate) fn with_extra_symbols(extra_symbols: &[&'static str]) -> Self {
        Self::prefill(ALL_NAMES, extra_symbols)
    }
}

symbols! {
    Empty {
        Empty: ""
    }
    Symbols {
        Gt: ">",
        Lt: "<",
        Ge: ">=",
        Le: "<=",
        Eq: "==",
        Ne: "!=",
        Assign: "=",
        Plus: "+",
        Minus: "-",
        Mul: "*",
        Div: "/",
        BitOr: "|",
    }
    Keywords {
        Let: "let",
        Const: "const",
        Fn: "fn",
        Struct: "struct",
        Enum: "enum",
        Trait: "trait",
        Impl: "impl",
        If: "if",
        Else: "else",
        While: "while",
        For: "for",
        Loop: "loop",
        Match: "match",
        Return: "return",
        Break: "break",
        Continue: "continue",
        Pub: "pub",
        Priv: "priv",
        Use: "use",
        Mod: "mod",
        Extern: "extern",
        Static: "static",
        Mut: "mut",
        Where: "where",
        True: "true",
        False: "false",
        In: "in",
    }
}

pub use sym::empty_generated::Empty;
pub use sym::keywords_generated as kw;
pub use sym::symbols_generated as symbols;

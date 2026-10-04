pub mod file;
pub mod symbol;

use crate::symbol::Interner;
pub use file::{FileId, SourceFile, SourceMap};
use std::{ops::Range, sync::Arc};
pub use symbol::Symbol;
pub use symbol::{kw, symbols};
pub use file::display_path;
use traversable::{Traversable, TraversableMut};

#[derive(Clone, Copy, PartialEq, Eq, Traversable, TraversableMut)]
#[traverse(skip_self)]
pub struct Span {
    #[traverse(skip)]
    pub file_id: FileId,
    pub start: usize,
    pub end: usize,
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self == DUMMY_SPAN {
            write!(f, "<dummy_span>")
        } else {
            write!(f, "{}..{}", self.start, self.end)
        }
    }
}

pub const DUMMY_SPAN: Span = Span {
    file_id: FileId(usize::MAX),
    start: usize::MAX,
    end: usize::MAX,
};

impl Span {
    pub fn is_dummy(&self) -> bool {
        *self == DUMMY_SPAN
    }

    pub fn extend(&self, other: Self) -> Self {
        assert!(!self.is_dummy() && !other.is_dummy());
        assert_eq!(self.file_id, other.file_id);
        assert!(self.start <= other.end, "Span order wrong");
        Self {
            file_id: self.file_id,
            start: self.start,
            end: other.end,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn new(file_id: FileId, start: usize, end: usize) -> Self {
        Self {
            file_id,
            start,
            end,
        }
    }
}

#[derive(Debug, Clone, Traversable, TraversableMut)]
pub struct Spanned<T: 'static + Traversable + TraversableMut> {
    pub value: T,
    pub span: Span,
}

impl<T: 'static + Traversable + TraversableMut> Spanned<T> {
    pub fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

/// 用于存放一些进程唯一的数据, 在不方便获取session时使用
pub struct SessionGlobals {
    /// 用于真的不能获取session的地方,尽量不使用
    pub source_map: Option<Arc<SourceMap>>,
    pub(crate) interner: Interner,
}

scoped_tls_hkt::scoped_thread_local!(pub(crate) static SESSION_GLOBALS: SessionGlobals);

#[inline]
pub fn with_session_globals<F, R>(f: F) -> R
where
    F: FnOnce(&SessionGlobals) -> R,
{
    SESSION_GLOBALS.with(f)
}

pub fn create_session_globals<F, R>(
    source_map: Option<Arc<SourceMap>>,
    extra_symbols: &[&'static str],
    f: F,
) -> R
where
    F: FnOnce() -> R,
{
    assert!(
        !SESSION_GLOBALS.is_set(),
        "SESSION_GLOBALS should never be overwriteen."
    );
    let session_globals = SessionGlobals {
        source_map,
        interner: Interner::with_extra_symbols(extra_symbols),
    };
    SESSION_GLOBALS.set(&session_globals, f)
}

pub fn set_session_globals<F, R>(session_globals: &SessionGlobals, f: F) -> R
where
    F: FnOnce() -> R,
{
    assert!(
        !SESSION_GLOBALS.is_set(),
        "SESSION_GLOBALS should never be overwriteen."
    );
    SESSION_GLOBALS.set(session_globals, f)
}

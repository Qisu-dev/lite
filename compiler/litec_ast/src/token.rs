use litec_span::{Span, Symbol};
use traversable::{Traversable, TraversableMut};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Token {
    pub span: Span,
    pub kind: TokenKind,
    pub text: Symbol,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span, text: Symbol) -> Self {
        Token { span, kind, text }
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Ident,

    Literal {
        kind: LiteralKind,
        suffix: Option<Symbol>,
    },

    DocComment,

    /// `;`
    Semi,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `::`
    PathAccess,
    /// `(`
    OpenParen,
    /// `)`
    CloseParen,
    /// `{`
    OpenBrace,
    /// `}`
    CloseBrace,
    /// `[`
    OpenBracket,
    /// `]`
    CloseBracket,
    /// `@`
    At,
    /// `#`
    Hash,
    /// `~`
    Tilde,
    /// `?`
    Question,
    /// `:`
    Colon,
    /// `$`
    Dollar,
    /// `=`
    Assign,
    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `!`
    Bang,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `-`
    Minus,
    /// `--`
    MinusMinus,
    /// `-=`
    MinusEq,
    /// `&`
    BitAnd,
    /// `&=`
    BitAndEq,
    /// `&&`
    And,
    /// `|`
    BitOr,
    /// `|=`
    BitOrEq,
    /// `||`
    Or,
    /// `+`
    Plus,
    /// `++`
    PlusPlus,
    /// `+=`
    PlusEq,
    /// `*`
    Mul,
    /// `*=`
    MulEq,
    /// `/`
    Div,
    /// `/=`
    DivEq,
    /// `^`
    BitXor,
    /// `^=`
    BitXorEq,
    /// `%`
    Remainder,
    /// `%=`
    RemainderEq,
    /// `->`
    Arrow,
    /// `=>`
    FatArrow,
    /// `..`
    To,
    /// `..=`
    ToEq,
    /// `...`
    Ellipsis,
    /// `<<`
    Shl,
    /// `<<=`
    ShlEq,
    /// `>>`
    Shr,
    /// `>>=`
    ShrEq,
    /// `_`
    Underscore,

    // Keyword
    Fn,
    Let,
    If,
    Else,
    While,
    Return,
    True,
    False,
    In,
    Struct,
    Loop,
    Break,
    Continue,
    Pub,
    Priv,
    Use,
    As,
    Extern,
    Mut,
    Mod,
    Super,
    Crate,
    /// self
    SelfLower,
    /// Self
    SelfUpper,
    Trait,
    Type,
    Impl,
    For,
    Match,
    Defer,
    Enum,
    Const,
    Static,
    Union,
    Where,
    Dyn,

    Eof,

    Whitespace,
    LineComment,  // //
    BlockComment, // /* */

    DocLineComment,       // ///
    DocBlockComment,      // /** */
    InnerDocLineComment,  // //!
    InnerDocBlockComment, // /*! */
}

impl TokenKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
        )
    }

    pub fn is_doc_comment(self) -> bool {
        matches!(
            self,
            TokenKind::DocLineComment
                | TokenKind::DocBlockComment
                | TokenKind::InnerDocLineComment
                | TokenKind::InnerDocBlockComment
        )
    }

    pub fn is_literal(self) -> bool {
        matches!(self, TokenKind::Literal { .. })
    }

    pub fn describe(self) -> &'static str {
        match self {
            TokenKind::Ident => "identifier",
            TokenKind::Literal { .. } => "literal",
            TokenKind::Underscore => "`_`",
            TokenKind::Eof => "end of file",

            TokenKind::Semi => "`;`",
            TokenKind::Comma => "`,`",
            TokenKind::Dot => "`.`",
            TokenKind::PathAccess => "`::`",
            TokenKind::OpenParen => "`(`",
            TokenKind::CloseParen => "`)`",
            TokenKind::OpenBrace => "`{`",
            TokenKind::CloseBrace => "`}`",
            TokenKind::OpenBracket => "`[`",
            TokenKind::CloseBracket => "`]`",
            TokenKind::At => "`@`",
            TokenKind::Hash => "`#`",
            TokenKind::Tilde => "`~`",
            TokenKind::Question => "`?`",
            TokenKind::Colon => "`:`",
            TokenKind::Dollar => "`$`",
            TokenKind::Assign => "`=`",
            TokenKind::EqEq => "`==`",
            TokenKind::NotEq => "`!=`",
            TokenKind::Bang => "`!`",
            TokenKind::Lt => "`<`",
            TokenKind::Le => "`<=`",
            TokenKind::Gt => "`>`",
            TokenKind::Ge => "`>=`",
            TokenKind::Minus => "`-`",
            TokenKind::MinusEq => "`-=`",
            TokenKind::MinusMinus => "`--`",
            TokenKind::BitAnd => "`&`",
            TokenKind::BitAndEq => "`&=`",
            TokenKind::And => "`&&`",
            TokenKind::BitOr => "`|`",
            TokenKind::BitOrEq => "`|=`",
            TokenKind::Or => "`||`",
            TokenKind::Plus => "`+`",
            TokenKind::PlusEq => "`+=`",
            TokenKind::PlusPlus => "`++`",
            TokenKind::Mul => "`*`",
            TokenKind::MulEq => "`*=`",
            TokenKind::Div => "`/`",
            TokenKind::DivEq => "`/=`",
            TokenKind::BitXor => "`^`",
            TokenKind::BitXorEq => "`^=`",
            TokenKind::Remainder => "`%`",
            TokenKind::RemainderEq => "`%=`",
            TokenKind::Arrow => "`->`",
            TokenKind::FatArrow => "`=>`",
            TokenKind::To => "`..`",
            TokenKind::ToEq => "`..=`",
            TokenKind::Ellipsis => "`...`",
            TokenKind::Shl => "`<<`",
            TokenKind::ShlEq => "`<<=`",
            TokenKind::Shr => "`>>`",
            TokenKind::ShrEq => "`>>=`",

            TokenKind::Fn => "`fn`",
            TokenKind::Let => "`let`",
            TokenKind::If => "`if`",
            TokenKind::Else => "`else`",
            TokenKind::While => "`while`",
            TokenKind::Return => "`return`",
            TokenKind::True => "`true`",
            TokenKind::False => "`false`",
            TokenKind::In => "`in`",
            TokenKind::Struct => "`struct`",
            TokenKind::Loop => "`loop`",
            TokenKind::Break => "`break`",
            TokenKind::Continue => "`continue`",
            TokenKind::Pub => "`pub`",
            TokenKind::Priv => "`priv`",
            TokenKind::Use => "`use`",
            TokenKind::As => "`as`",
            TokenKind::Extern => "`extern`",
            TokenKind::Mut => "`mut`",
            TokenKind::Mod => "`mod`",
            TokenKind::Super => "`super`",
            TokenKind::Crate => "`crate`",
            TokenKind::SelfLower => "`self`",
            TokenKind::SelfUpper => "`Self`",
            TokenKind::Trait => "`trait`",
            TokenKind::Type => "`type`",
            TokenKind::Impl => "`impl`",
            TokenKind::For => "`for`",
            TokenKind::Match => "`match`",
            TokenKind::Defer => "`defer`",
            TokenKind::Enum => "`enum`",
            TokenKind::Const => "`const`",
            TokenKind::Static => "`static`",
            TokenKind::Union => "`union`",
            TokenKind::Where => "`where`",

            TokenKind::Whitespace => "whitespace",
            TokenKind::LineComment => "line comment",
            TokenKind::BlockComment => "block comment",
            TokenKind::DocLineComment => "doc comment",
            TokenKind::DocBlockComment => "doc comment",
            TokenKind::InnerDocLineComment => "inner doc comment",
            TokenKind::InnerDocBlockComment => "inner doc comment",
            TokenKind::DocComment => "doc comment",
            TokenKind::Dyn => "`dyn`",
        }
    }

    pub fn is_pat_expr_start(self) -> bool {
        matches!(
            self,
            TokenKind::Literal { .. }
                | TokenKind::Ident
                | TokenKind::SelfLower
                | TokenKind::SelfUpper
                | TokenKind::Crate
                | TokenKind::Super
                | TokenKind::PathAccess
                | TokenKind::Minus
                | TokenKind::OpenParen
                | TokenKind::OpenBracket
        )
    }

    pub fn is_path_start(self) -> bool {
        matches!(
            self,
            TokenKind::Ident
                | TokenKind::SelfLower
                | TokenKind::SelfUpper
                | TokenKind::Crate
                | TokenKind::Super
                | TokenKind::PathAccess
        )
    }

    pub fn is_item_start(self) -> bool {
        matches!(
            self,
            TokenKind::Fn
                | TokenKind::Struct
                | TokenKind::Enum
                | TokenKind::Trait
                | TokenKind::Impl
                | TokenKind::Mod
                | TokenKind::Use
                | TokenKind::Const
                | TokenKind::Static
                | TokenKind::Type
                | TokenKind::Union
                | TokenKind::Extern
                | TokenKind::Pub
                | TokenKind::Priv
        )
    }
}

#[macro_export]
macro_rules! tok {
    (>>=) => {
        $crate::TokenKind::ShrEq
    };
    (<<=) => {
        $crate::TokenKind::ShlEq
    };
    (...) => {
        $crate::TokenKind::Ellipsis
    };
    (..=) => {
        $crate::TokenKind::ToEq
    };

    (>>) => {
        $crate::TokenKind::Shr
    };
    (<<) => {
        $crate::TokenKind::Shl
    };
    (..) => {
        $crate::TokenKind::To
    };
    (->) => {
        $crate::TokenKind::Arrow
    };
    (=>) => {
        $crate::TokenKind::FatArrow
    };
    (::) => {
        $crate::TokenKind::PathAccess
    };
    (==) => {
        $crate::TokenKind::EqEq
    };
    (!=) => {
        $crate::TokenKind::NotEq
    };
    (<=) => {
        $crate::TokenKind::Le
    };
    (>=) => {
        $crate::TokenKind::Ge
    };
    (-=) => {
        $crate::TokenKind::MinusEq
    };
    (--) => {
        $crate::TokenKind::MinusMinus
    };
    (+=) => {
        $crate::TokenKind::PlusEq
    };
    (++) => {
        $crate::TokenKind::PlusPlus
    };
    (*=) => {
        $crate::TokenKind::MulEq
    };
    (/=) => {
        $crate::TokenKind::DivEq
    };
    (%=) => {
        $crate::TokenKind::RemainderEq
    };
    (&=) => {
        $crate::TokenKind::BitAndEq
    };
    (&&) => {
        $crate::TokenKind::And
    };
    (|=) => {
        $crate::TokenKind::BitOrEq
    };
    (||) => {
        $crate::TokenKind::Or
    };
    (^=) => {
        $crate::TokenKind::BitXorEq
    };

    (;) => {
        $crate::TokenKind::Semi
    };
    (,) => {
        $crate::TokenKind::Comma
    };
    (.) => {
        $crate::TokenKind::Dot
    };
    ($) => {
        $crate::TokenKind::Dollar
    };
    (OpenParen) => {
        $crate::TokenKind::OpenParen
    };
    (CloseParen) => {
        $crate::TokenKind::CloseParen
    };
    (OpenBrace) => {
        $crate::TokenKind::OpenBrace
    };
    (CloseBrace) => {
        $crate::TokenKind::CloseBrace
    };
    (OpenBracket) => {
        $crate::TokenKind::OpenBracket
    };
    (CloseBracket) => {
        $crate::TokenKind::CloseBracket
    };
    (@) => {
        $crate::TokenKind::At
    };
    (#) => {
        $crate::TokenKind::Hash
    };
    (~) => {
        $crate::TokenKind::Tilde
    };
    (?) => {
        $crate::TokenKind::Question
    };
    (:) => {
        $crate::TokenKind::Colon
    };
    (=) => {
        $crate::TokenKind::Assign
    };
    (!) => {
        $crate::TokenKind::Bang
    };
    (<) => {
        $crate::TokenKind::Lt
    };
    (>) => {
        $crate::TokenKind::Gt
    };
    (-) => {
        $crate::TokenKind::Minus
    };
    (&) => {
        $crate::TokenKind::BitAnd
    };
    (|) => {
        $crate::TokenKind::BitOr
    };
    (+) => {
        $crate::TokenKind::Plus
    };
    (*) => {
        $crate::TokenKind::Mul
    };
    (/) => {
        $crate::TokenKind::Div
    };
    (^) => {
        $crate::TokenKind::BitXor
    };
    (%) => {
        $crate::TokenKind::Remainder
    };
    (_) => {
        $crate::TokenKind::Underscore
    };

    // 关键字
    (fn) => {
        $crate::TokenKind::Fn
    };
    (let) => {
        $crate::TokenKind::Let
    };
    (if) => {
        $crate::TokenKind::If
    };
    (else) => {
        $crate::TokenKind::Else
    };
    (while) => {
        $crate::TokenKind::While
    };
    (return) => {
        $crate::TokenKind::Return
    };
    (true) => {
        $crate::TokenKind::True
    };
    (false) => {
        $crate::TokenKind::False
    };
    (in) => {
        $crate::TokenKind::In
    };
    (struct) => {
        $crate::TokenKind::Struct
    };
    (loop) => {
        $crate::TokenKind::Loop
    };
    (break) => {
        $crate::TokenKind::Break
    };
    (continue) => {
        $crate::TokenKind::Continue
    };
    (pub) => {
        $crate::TokenKind::Pub
    };
    (priv) => {
        $crate::TokenKind::Priv
    };
    (use) => {
        $crate::TokenKind::Use
    };
    (as) => {
        $crate::TokenKind::As
    };
    (extern) => {
        $crate::TokenKind::Extern
    };
    (mut) => {
        $crate::TokenKind::Mut
    };
    (mod) => {
        $crate::TokenKind::Mod
    };
    (super) => {
        $crate::TokenKind::Super
    };
    (crate) => {
        $crate::TokenKind::Crate
    };
    (self) => {
        $crate::TokenKind::SelfLower
    };
    (Self) => {
        $crate::TokenKind::SelfUpper
    };
    (trait) => {
        $crate::TokenKind::Trait
    };
    (type) => {
        $crate::TokenKind::Type
    };
    (impl) => {
        $crate::TokenKind::Impl
    };
    (for) => {
        $crate::TokenKind::For
    };
    (match) => {
        $crate::TokenKind::Match
    };
    (defer) => {
        $crate::TokenKind::Defer
    };
    (enum) => {
        $crate::TokenKind::Enum
    };
    (const) => {
        $crate::TokenKind::Const
    };
    (static) => {
        $crate::TokenKind::Static
    };
    (union) => {
        $crate::TokenKind::Union
    };
    (where) => {
        $crate::TokenKind::Where
    };
    (dyn) => {
        $crate::TokenKind::Dyn
    };
    (Eof) => {
        $crate::TokenKind::Eof
    };
    (Whitespace) => {
        $crate::TokenKind::Whitespace
    };
    (LineComment) => {
        $crate::TokenKind::LineComment
    };
    (BlockComment) => {
        $crate::TokenKind::BlockComment
    };
    (DocLineComment) => {
        $crate::TokenKind::DocLineComment
    };
    (DocBlockComment) => {
        $crate::TokenKind::DocBlockComment
    };
    (InnerDocLineComment) => {
        $crate::TokenKind::InnerDocLineComment
    };
    (InnerDocBlockComment) => {
        $crate::TokenKind::InnerDocBlockComment
    };
    (Ident) => {
        $crate::TokenKind::Ident
    };
}

mod asserts {
    /// 编译期验证 `Token![...]` 映射到指定的 `TokenKind` 变体
    #[macro_export]
    macro_rules! const_assert_token {
        ([$($tok:tt)*] => $variant:ident) => {
            const _: () = assert!(matches!(
                $crate::tok!($($tok)*),
                $crate::TokenKind::$variant
            ));
        };
    }

    const_assert_token!([_] => Underscore);
    const_assert_token!([,] => Comma);
    const_assert_token!([;] => Semi);
    const_assert_token!([->] => Arrow);
    const_assert_token!([=>] => FatArrow);
    const_assert_token!([==] => EqEq);
    const_assert_token!([=] => Assign);
    const_assert_token!([..] => To);
    const_assert_token!([...] => Ellipsis);
    const_assert_token!([OpenParen] => OpenParen);
    const_assert_token!([CloseBrace] => CloseBrace);

    const_assert_token!([fn] => Fn);
    const_assert_token!([let] => Let);
    const_assert_token!([self] => SelfLower);
    const_assert_token!([Self] => SelfUpper);
    const_assert_token!([Ident] => Ident);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Traversable, TraversableMut)]
#[traverse(skip_self)]
pub enum LiteralKind {
    /// 整数：`42`、`0xFF`、`0b1010`、`0o777`
    Integer { base: Radix },
    /// 浮点：`3.14`、`.5`、`1e10`
    Float,
    /// 字符：`'a'`、`'\n'`、`'\u{4E2D}'`
    Char,
    /// 字符串：`"hello"`
    Str,
    /// 原始字符串： `hello`
    RawStr,
    /// 字节：`b'a'`
    Byte,
    /// 字节串：`b"hello"`
    BStr,
    /// C 字符：`c'a'`
    CChar,
    /// C 字符串：`c"hello"`
    CStr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Traversable, TraversableMut)]
#[traverse(skip_self)]
pub enum Radix {
    Bin = 2,
    Oct = 8,
    Dec = 10,
    Hex = 16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Traversable, TraversableMut)]
pub struct Lit {
    pub kind: LiteralKind,
    pub value: Symbol,
    pub suffix: Option<Symbol>,
    pub span: Span,
}

use litec_ast::{
    TokenKind, tok,
    token::{LiteralKind, Radix, Token},
};
use litec_error::{Diag, PResult};
use litec_session::Session;
use litec_span::{FileId, Span, Symbol};

#[inline]
fn is_ident_start(c: char) -> bool {
    c == '_' || unicode_ident::is_xid_start(c)
}

#[inline]
fn is_ident_continue(c: char) -> bool {
    unicode_ident::is_xid_continue(c)
}

#[inline]
fn fix_full_width_char(c: char) -> Option<String> {
    let code = c as u32;

    if (0xFF01..=0xFF5E).contains(&code) {
        return char::from_u32(code - 0xFEE0).map(String::from);
    }

    Some(
        match c {
            '\u{3000}' => " ",
            '\u{3001}' => ",",
            '\u{3002}' => ".",
            '\u{300A}' => "<",
            '\u{300B}' => ">",
            '\u{300C}' => "[",
            '\u{300D}' => "]",
            '\u{3010}' => "[",
            '\u{3011}' => "]",
            '\u{201C}' | '\u{201D}' => "\"",
            '\u{2018}' | '\u{2019}' => "'",
            '\u{2014}' => "-",
            '\u{2026}' => "...",
            _ => return None,
        }
        .to_owned(),
    )
}

#[inline]
fn is_digit_or_underscore(c: char) -> bool {
    c.is_ascii_digit() || c == '_'
}

#[inline]
fn radix_prefix(c: char) -> Option<Radix> {
    use Radix::*;
    match c {
        'x' | 'X' => Some(Hex),
        'b' | 'B' => Some(Bin),
        'o' | 'O' => Some(Oct),
        _ => None,
    }
}

#[inline]
fn is_digit_for_radix(c: char, base: Radix) -> bool {
    use Radix::*;
    match base {
        Bin => c == '0' || c == '1',
        Oct => ('0'..='7').contains(&c),
        Hex => c.is_ascii_hexdigit(),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Lexer<'a, 'src> {
    /// 编译谈话
    /// 可以获取 dcx
    session: &'a Session,
    /// 读的源代码
    src: &'src str,
    /// 已经读到的 **字节长度**
    pos: usize,
    /// 文件id, 用于生成span
    file_id: FileId,
    /// 当前token开头
    start: usize,
}

#[derive(Clone, Copy)]
pub struct LexerCheckpoint {
    pos: usize,
    start: usize,
}

impl<'a, 'src> Lexer<'a, 'src> {
    pub fn new(session: &'a Session, src: &'src str, file_id: FileId) -> Self {
        Self {
            session,
            src,
            file_id,
            pos: 0,
            start: 0,
        }
    }

    pub(crate) fn advance(&mut self) -> PResult<Token> {
        self.start = self.pos;

        let Some(first) = self.peek() else {
            return Ok(self.mk_tok(tok!(Eof)));
        };

        if first.is_whitespace() {
            if first == '\u{3000}' {
                let ws_end = self.start + '\u{3000}'.len_utf8();
                let span = Span::new(self.file_id, self.start, ws_end);

                self.session.emit_warn(
                    Diag::warning("unexpected fullwidth space")
                        .with_span(self.mk_span())
                        .with_help("use ASCII space (U+0020) instead")
                        .with_suggestion(span, " ", "ASCII space"),
                );
            }
            self.bump_if(|c| c.is_whitespace());
            return Ok(self.mk_tok(tok!(Whitespace)));
        }

        if self.rest().starts_with("//") {
            self.bump();
            self.bump();
            let kind = self.lex_line_comment();
            return Ok(self.mk_tok(kind));
        }

        if self.rest().starts_with("/*") {
            self.bump();
            self.bump();
            return self.lex_block_comment();
        }

        if let Some(kind) = self.peek_str_literal() {
            return self.lex_str_literal(kind);
        }

        if is_ident_start(first) {
            self.bump();
            return self.lex_ident();
        }

        if first == '.' && self.peek_n(1).is_some_and(|c| c.is_ascii_digit()) {
            return self.lex_number(); // `.5`——lex_number 内部 bump
        }
        if first.is_ascii_digit() {
            return self.lex_number(); // `123`——lex_number 内部 bump
        }

        self.lex_punct(first)
    }

    fn lex_str_literal(&mut self, kind: LiteralKind) -> PResult<Token> {
        let has_prefix = matches!(
            kind,
            LiteralKind::BStr | LiteralKind::Byte | LiteralKind::CStr | LiteralKind::CChar
        );
        if has_prefix {
            self.bump();
        }

        // 消费开引号
        // SAFETY: peek_literal 保证有引号
        let quote = self.peek().unwrap();
        self.bump();

        // 扫描行为
        let is_raw = matches!(kind, LiteralKind::RawStr);
        let check_ascii = has_prefix; // b/c 前缀的都必须 ASCII

        let terminated = self.lex_str_literal_body(quote, is_raw, check_ascii);

        if !terminated {
            let (msg, help) = match quote {
                '`' => ("unclosed raw string", "add a closing backtick `` ` ``"),
                '"' => ("unclosed string", "add a closing double quote `\"`"),
                '\'' => (
                    "unclosed character literal",
                    "add a closing single quote `'`",
                ),
                _ => unreachable!(),
            };
            return Err(self
                .session
                .emit_err(Diag::error(msg).with_span(self.mk_span()).with_help(help)));
        }

        let suffix = if is_raw { None } else { self.lex_suffix() };

        Ok(self.mk_tok(TokenKind::Literal { kind, suffix }))
    }

    /// 从开引号后扫到匹配的结束引号
    ///
    /// - `quote`：结束引号字符（`"` / `'` / `` ` ``）
    /// - `is_raw`：raw 模式——不处理转义、允许跨行
    /// - `check_ascii`：字节串/C 字符串——非 ASCII 报错
    ///
    /// 返回：是否找到结束引号
    fn lex_str_literal_body(&mut self, quote: char, is_raw: bool, check_ascii: bool) -> bool {
        loop {
            match self.bump() {
                // EOF——未闭合
                None => return false,

                // 普通字符串不跨行；raw 可以
                Some('\n') if !is_raw => return false,

                // 结束引号
                Some(c) if c == quote => return true,

                // 转义——跳过下一个字符（raw 模式不转义）
                Some('\\') if !is_raw => {
                    self.bump(); // 消费转义后的字符
                }

                Some(c) if check_ascii && (c as u32) > 0x7F => {
                    let c_start = self.pos - c.len_utf8();
                    let c_span = Span::new(self.file_id, c_start, self.pos);
                    self.session.emit_err(
                        Diag::error(format!("non-ASCII character `{}` in byte string", c))
                            .with_span(c_span)
                            .with_help("use `\\xXX` escape, or a regular string"),
                    );
                }

                Some(_) => {}
            }
        }
    }

    /// 判断当前位置是不是字面量的开头不消费任何字符
    fn peek_str_literal(&self) -> Option<LiteralKind> {
        let first = self.peek()?;
        let second = self.peek_n(1);

        match (first, second) {
            ('"', _) => Some(LiteralKind::Str),
            ('\'', _) => Some(LiteralKind::Char),
            ('`', _) => Some(LiteralKind::RawStr),

            ('b', Some('"')) => Some(LiteralKind::BStr),
            ('b', Some('\'')) => Some(LiteralKind::Byte),

            ('c', Some('"')) => Some(LiteralKind::CStr),
            ('c', Some('\'')) => Some(LiteralKind::CChar),

            _ => None,
        }
    }

    fn lex_number(&mut self) -> PResult<Token> {
        let starts_with_dot = self.src.as_bytes()[self.start as usize] == b'.';

        self.bump();

        let kind = if starts_with_dot {
            self.bump_if(is_digit_or_underscore);
            self.lex_exponent();
            LiteralKind::Float
        } else if self.src.as_bytes()[self.start as usize] == b'0'
            && let Some(base) = self.peek().and_then(radix_prefix)
        {
            // 有进制前缀
            self.bump(); // 消费 x/b/o
            self.bump_if(|c| is_digit_for_radix(c, base) || c == '_');
            LiteralKind::Integer { base }
        } else {
            // 十进制
            self.bump_if(is_digit_or_underscore);

            let mut is_float = false;
            if self.peek() == Some('.') && self.peek_n(1).is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
                self.bump_if(is_digit_or_underscore);
                is_float = true;
            }

            if self.lex_exponent() {
                is_float = true;
            }

            if is_float {
                LiteralKind::Float
            } else {
                LiteralKind::Integer { base: Radix::Dec }
            }
        };

        let suffix = self.lex_suffix();
        Ok(self.mk_tok(TokenKind::Literal { kind, suffix }))
    }

    /// 扫描 `e10` / `E-5` 这样的指数返回是否真的扫到了指数
    fn lex_exponent(&mut self) -> bool {
        if !matches!(self.peek(), Some('e') | Some('E')) {
            return false;
        }
        self.bump();
        if matches!(self.peek(), Some('+') | Some('-')) {
            self.bump();
        }
        self.bump_if(is_digit_or_underscore);
        true
    }

    /// 扫描可选后缀：`i32`、`u64`、`f32` 等
    fn lex_suffix(&mut self) -> Option<Symbol> {
        let suffix_start = self.pos;

        // 后缀以字母或 `_` 开头
        let c = self.peek()?;
        if !c.is_ascii_alphabetic() && c != '_' {
            return None;
        }

        self.bump_if(|c| c.is_ascii_alphanumeric() || c == '_');

        let text = &self.src[suffix_start as usize..self.pos as usize];
        Some(Symbol::intern(text))
    }

    fn lex_line_comment(&mut self) -> TokenKind {
        // 已消费 `//`
        let kind = match self.peek() {
            Some('/') => {
                self.bump();
                tok!(DocLineComment)
            }
            Some('!') => {
                self.bump();
                tok!(InnerDocLineComment)
            }
            _ => tok!(LineComment),
        };
        self.bump_if(|c| c != '\n' && c != '\r');
        kind
    }

    fn lex_punct(&mut self, first: char) -> PResult<Token> {
        self.bump();

        let kind = match first {
            '(' => tok!(OpenParen),
            ')' => tok!(CloseParen),
            '{' => tok!(OpenBrace),
            '}' => tok!(CloseBrace),
            '[' => tok!(OpenBracket),
            ']' => tok!(CloseBracket),
            ';' => tok!(;),
            ',' => tok!(,),
            '@' => tok!(@),
            '#' => tok!(#),
            '~' => tok!(~),
            '?' => tok!(?),
            '$' => tok!($),

            ':' => {
                if self.eat(':') {
                    tok!(::)
                } else {
                    tok!(:)
                }
            }
            '!' => {
                if self.eat('=') {
                    tok!(!=)
                } else {
                    tok!(!)
                }
            }
            '*' => {
                if self.eat('=') {
                    tok!(*=)
                } else {
                    tok!(*)
                }
            }
            '/' => {
                if self.eat('=') {
                    tok!(/=)
                } else {
                    tok!(/)
                }
            }
            '^' => {
                if self.eat('=') {
                    tok!(^=)
                } else {
                    tok!(^)
                }
            }
            '%' => {
                if self.eat('=') {
                    tok!(%=)
                } else {
                    tok!(%)
                }
            }
            '=' => {
                if self.eat('=') {
                    tok!(==)
                } else if self.eat('>') {
                    tok!(=>)
                } else {
                    tok!(=)
                }
            }
            '-' => {
                if self.eat('>') {
                    tok!(->)
                } else if self.eat('=') {
                    tok!(-=)
                } else if self.eat('-') {
                    tok!(--)
                } else {
                    tok!(-)
                }
            }
            '+' => {
                if self.eat('=') {
                    tok!(+=)
                } else if self.eat('+') {
                    tok!(++)
                } else {
                    tok!(+)
                }
            }
            '&' => {
                if self.eat('&') {
                    tok!(&&)
                } else if self.eat('=') {
                    tok!(&=)
                } else {
                    tok!(&)
                }
            }
            '|' => {
                if self.eat('|') {
                    tok!(||)
                } else if self.eat('=') {
                    tok!(|=)
                } else {
                    tok!(|)
                }
            }
            '.' => {
                if self.eat('.') {
                    if self.eat('.') {
                        tok!(...)
                    } else if self.eat('=') {
                        tok!(..=)
                    } else {
                        tok!(..)
                    }
                } else {
                    tok!(.)
                }
            }
            '<' => {
                if self.eat('<') {
                    if self.eat('=') { tok!(<<=) } else { tok!(<<) }
                } else if self.eat('=') {
                    tok!(<=)
                } else {
                    tok!(<)
                }
            }
            '>' => {
                if self.eat('>') {
                    if self.eat('=') { tok!(>>=) } else { tok!(>>) }
                } else if self.eat('=') {
                    tok!(>=)
                } else {
                    tok!(>)
                }
            }

            _ => {
                if let Some(fixed_char) = fix_full_width_char(first) {
                    return Err(self.session.emit_err(
                        Diag::error(format!("unexpected fullwidth char `{}`", first))
                            .with_span(self.mk_span())
                            .with_help(format!("use `{}` instead", fixed_char))
                            .with_suggestion(self.mk_span(), fixed_char, "fixed char"),
                    ));
                }
                return Err(self.session.emit_err(
                    Diag::error(format!(
                        "unknown character `{}` (U+{:04x})",
                        first, first as u32
                    ))
                    .with_span(self.mk_span())
                    .with_help(format!("`{}` is not a valid token", first)),
                ));
            }
        };

        Ok(self.mk_tok(kind))
    }

    fn lex_ident(&mut self) -> PResult<Token> {
        self.bump_if(is_ident_continue);

        let text = &self.src[self.start as usize..self.pos as usize];

        let kind = match text {
            "_" => tok!(_),
            "fn" => tok!(fn),
            "let" => tok!(let),
            "if" => tok!(if),
            "else" => tok!(else),
            "while" => tok!(while),
            "return" => tok!(return),
            "true" => tok!(true),
            "false" => tok!(false),
            "in" => tok!(in),
            "struct" => tok!(struct),
            "loop" => tok!(loop),
            "break" => tok!(break),
            "continue" => tok!(continue),
            "pub" => tok!(pub),
            "priv" => tok!(priv),
            "use" => tok!(use),
            "as" => tok!(as),
            "extern" => tok!(extern),
            "mut" => tok!(mut),
            "mod" => tok!(mod),
            "super" => tok!(super),
            "crate" => tok!(crate),
            "self" => tok!(self),
            "Self" => tok!(Self),
            "trait" => tok!(trait),
            "type" => tok!(type),
            "impl" => tok!(impl),
            "for" => tok!(for),
            "match" => tok!(match),
            "defer" => tok!(defer),
            "enum" => tok!(enum),
            "const" => tok!(const),
            "static" => tok!(static),
            "union" => tok!(union),
            "where" => tok!(where),
            "dyn" => tok!(dyn),

            _ => tok!(Ident),
        };

        Ok(self.mk_tok(kind))
    }

    /// 进入时：已消费 `/*`，`self.pos` 在第三个字符位置
    /// 返回：注释的 `TokenKind`（`BlockComment` / `DocBlockComment` / `InnerDocBlockComment`）
    ///
    /// 处理嵌套：所有块注释共享 `depth`，`/*` 加一，`*/` 减一
    fn lex_block_comment(&mut self) -> PResult<Token> {
        // 判断类型：/** 或 /*! 或 /*
        let kind = match self.peek() {
            Some('*') if self.peek_n(1) != Some('/') => {
                self.bump(); // 第二个 '*'
                tok!(DocBlockComment)
            }
            Some('!') => {
                self.bump(); // '!'
                tok!(InnerDocBlockComment)
            }
            _ => tok!(BlockComment),
        };

        let mut depth = 1u32;

        while depth > 0 {
            let Some(c) = self.bump() else {
                self.session
                    .dcx()
                    .emit_err(Diag::error("unterminated block comment").with_span(self.mk_span()));
                break;
            };

            match c {
                '/' if self.peek() == Some('*') => {
                    self.bump();
                    depth += 1;
                }
                '*' if self.peek() == Some('/') => {
                    self.bump();
                    depth -= 1;
                }
                _ => {}
            }
        }

        Ok(self.mk_tok(kind))
    }

    fn eat(&mut self, c: char) -> bool {
        if let Some(char) = self.peek()
            && char == c
        {
            self.bump();
            true
        } else {
            false
        }
    }

    fn mk_tok(&self, kind: TokenKind) -> Token {
        Token::new(
            kind,
            self.mk_span(),
            Symbol::intern(&self.src[self.start..self.pos]),
        )
    }

    fn mk_span(&self) -> Span {
        Span::new(self.file_id, self.start, self.pos)
    }

    fn bump_if<F>(&mut self, f: F)
    where
        F: Fn(char) -> bool,
    {
        while let Some(c) = self.peek() {
            if !f(c) {
                break;
            }
            self.bump();
        }
    }

    #[inline]
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    #[inline]
    fn peek_n(&self, n: usize) -> Option<char> {
        self.rest().chars().nth(n)
    }

    #[inline]
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    #[inline]
    fn rest(&self) -> &'src str {
        &self.src[self.pos..]
    }

    #[inline]
    fn is_eof(&self) -> bool {
        self.rest().is_empty()
    }

    pub(crate) fn checkpoint(&self) -> LexerCheckpoint {
        LexerCheckpoint {
            pos: self.pos,
            start: self.start,
        }
    }

    pub(crate) fn restore(&mut self, cp: LexerCheckpoint) {
        self.pos = cp.pos;
        self.start = cp.start;
    }
}

pub fn tokenize(session: &Session, src: &str, file_id: FileId) -> Vec<Token> {
    let mut lexer = Lexer::new(session, src, file_id);
    let mut tokens = Vec::new();

    loop {
        let before = lexer.pos;
        match lexer.advance() {
            Ok(t) => {
                let is_eof = t.kind == tok!(Eof);
                tokens.push(t);
                if is_eof {
                    return tokens;
                }
            }
            Err(_) => {
                if lexer.pos == before {
                    debug_assert!(false, "advance returned Err without advancing");
                    if lexer.bump().is_none() {
                        return tokens;
                    }
                }
            }
        }
    }
}

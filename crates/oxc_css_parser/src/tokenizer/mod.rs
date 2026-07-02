use crate::{
    config::{Syntax, TemplatePlaceholder},
    error::{Error, ErrorKind, PResult},
    pos::Span,
};
use oxc_allocator::{Allocator, Vec as ArenaVec};
use std::{cmp::Ordering, iter::Peekable, str::CharIndices};
pub(crate) use symbol::TokenSymbol;
use token::*;
pub use token::{Token, TokenWithSpan};

mod convert;
mod misc;
mod symbol;
pub mod token;

#[derive(Clone)]
pub(crate) struct TokenizerState<'a> {
    chars: Peekable<CharIndices<'a>>,
    current_indent: u16,
    indents: Vec<u16>,
    /// Depth of `(...)` nesting. In the indented syntax, newlines inside a group
    /// are insignificant (multi-line param/arg lists), so indentation tracking is
    /// suspended while this is non-zero.
    paren_depth: u32,
}

pub struct Tokenizer<'a> {
    source: &'a str,
    syntax: Syntax,
    template_placeholder: Option<TemplatePlaceholder>,
    comments: Option<ArenaVec<'a, Comment<'a>>>,
    pub(crate) state: TokenizerState<'a>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(
        allocator: &'a Allocator,
        source: &'a str,
        syntax: Syntax,
        template_placeholder: Option<TemplatePlaceholder>,
        collect_comments: bool,
    ) -> Self {
        let mut chars = source.char_indices().peekable();
        if syntax == Syntax::Sass {
            while chars.next_if(|(_, c)| matches!(c, '\n' | '\r')).is_some() {}
        }
        Self {
            source,
            syntax,
            template_placeholder,
            comments: collect_comments.then(|| ArenaVec::new_in(&allocator)),
            state: TokenizerState {
                chars,
                current_indent: 0,
                indents: if syntax == Syntax::Sass { vec![0] } else { vec![] },
                paren_depth: 0,
            },
        }
    }

    #[inline]
    pub fn comments(&self) -> &[Comment<'a>] {
        match &self.comments {
            Some(comments) => comments,
            None => &[],
        }
    }

    #[inline]
    pub(crate) fn comments_count(&self) -> usize {
        self.comments.as_ref().map_or(0, |comments| comments.len())
    }

    #[inline]
    pub(crate) fn truncate_comments(&mut self, len: usize) {
        if let Some(comments) = &mut self.comments {
            comments.truncate(len);
        }
    }

    /// Dedenting to a level between two known ones (`a,\n    b\n  c: d` — the
    /// continuation sat at 4, the block at 2) pops past the new level, leaving
    /// the current line deeper than the stack top with no `Indent` emitted.
    /// When the parser is about to open a block there, it can re-open that
    /// level so the block's closing `Dedent` is emitted later. Returns whether
    /// a level was opened.
    pub(crate) fn reopen_indent_level(&mut self) -> bool {
        if self.syntax == Syntax::Sass
            && self.state.paren_depth == 0
            && self.state.indents.last().copied().unwrap_or_default() < self.state.current_indent
        {
            self.state.indents.push(self.state.current_indent);
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn bump(&mut self) -> PResult<TokenWithSpan<'a>> {
        if let Some(indent) = self.skip_ws_or_comment() { Ok(indent) } else { self.next() }
    }

    #[inline]
    pub fn bump_without_ws_or_comments(&mut self) -> PResult<TokenWithSpan<'a>> {
        self.next()
    }

    pub fn current_offset(&mut self) -> usize {
        if let Some((offset, _)) = self.state.chars.peek() { *offset } else { self.source.len() }
    }

    #[inline]
    fn peek_two_chars(&self) -> Option<(usize, char, char)> {
        let mut iter = self.state.chars.clone();
        iter.next().zip(iter.next()).map(|((start, first), (_, second))| (start, first, second))
    }

    #[cold]
    fn build_eof_error(&mut self) -> Error {
        let offset = self.current_offset();
        Error { kind: ErrorKind::UnexpectedEof, span: Span { start: offset, end: offset } }
    }

    fn next(&mut self) -> PResult<TokenWithSpan<'a>> {
        // detect frequent tokens here, but DO NOT add too many and don't forget to do profiling
        match self.state.chars.peek() {
            Some((_, c)) if is_start_of_ident(*c) && c != &'-' => return self.scan_ident(),
            Some((_, c)) if c.is_ascii_digit() => {
                let (number, span) = self.scan_number()?;
                return self.scan_dimension_or_percentage(number, span);
            }
            Some((start, '{')) => {
                // In the indented syntax a lone `{` only occurs when an
                // interpolation resumes inside a string template (its `#` was
                // consumed by the string scanner); pair it with the `}`
                // decrement so bracket depth stays balanced.
                if self.syntax == Syntax::Sass {
                    self.state.paren_depth += 1;
                }
                let token = TokenWithSpan {
                    token: Token::LBrace(LBrace {}),
                    span: Span { start: *start, end: start + 1 },
                };
                self.state.chars.next();
                return Ok(token);
            }
            _ => {}
        }
        let mut chars = self.state.chars.clone();
        match (chars.next(), chars.next()) {
            (Some((_, '#')), Some((_, c)))
                if c.is_ascii_alphanumeric()
                    || c == '-'
                    || c == '_'
                    || !c.is_ascii()
                    || c == '\\' =>
            {
                self.scan_hash()
            }
            (Some((_, '\'' | '"')), ..) => self.scan_string_or_template(),
            (Some((_, '@')), Some((_, c)))
                // A leading `-` only starts an identifier if the next code point does too
                // (`@-webkit-*`, `@--custom`); a lone `@-` is a delimiter, not an at-keyword.
                // Less also allows digit-led variable names (`@3`).
                if (is_start_of_ident(c)
                    && (c != '-' || matches!(chars.peek(), Some((_, c2)) if is_start_of_ident(*c2))))
                    || (self.syntax == Syntax::Less && c.is_ascii_digit()) =>
            {
                self.scan_at_keyword()
            }
            (Some((start, '-')), Some((_, '-'))) => {
                if matches!(chars.peek(), Some((_, '>'))) {
                    self.scan_cdc(start)
                } else {
                    self.scan_ident()
                }
            }
            (Some((_, '-')), Some((_, c))) if is_start_of_ident(c) => self.scan_ident(),
            (Some((_, '.' | '+' | '-')), Some((_, c))) if c.is_ascii_digit() => {
                let (number, span) = self.scan_number()?;
                self.scan_dimension_or_percentage(number, span)
            }
            (Some((_, '-' | '+')), Some((_, '.'))) if matches!(chars.peek(), Some((_, c)) if c.is_ascii_digit()) =>
            {
                let (number, span) = self.scan_number()?;
                self.scan_dimension_or_percentage(number, span)
            }
            (Some((_, '$')), Some((_, c)))
                // Same as `@`: a lone `$-` is not the start of a Sass variable.
                if is_start_of_ident(c)
                    && (c != '-' || matches!(chars.peek(), Some((_, c2)) if is_start_of_ident(*c2))) =>
            {
                self.scan_dollar_var()
            }
            (Some((_, '-')), Some((_, '#')))
                if matches!(self.syntax, Syntax::Scss | Syntax::Sass)
                    && matches!(chars.peek(), Some((_, '{'))) =>
            {
                self.scan_sass_single_hyphen_as_ident()
            }
            (Some((_, '@' | '$')), Some((_, '{')))
                if self.syntax == Syntax::Less
                    && matches!(chars.peek(), Some((_, c)) if is_start_of_ident(*c) || c.is_ascii_digit()) =>
            {
                self.scan_less_lbrace_var()
            }
            (Some((_, '`')), _) if self.template_placeholder.is_some() => {
                self.scan_template_placeholder()
            }
            (Some((_, '`')), _) if self.syntax == Syntax::Less => self.scan_backtick_code(),
            (Some(..), ..) => self.scan_punc(),
            (None, ..) => {
                let offset = self.current_offset();
                Ok(TokenWithSpan {
                    token: Token::Eof(Eof {}),
                    span: Span { start: offset, end: offset },
                })
            }
        }
    }

    fn skip_ws_or_comment(&mut self) -> Option<TokenWithSpan<'a>> {
        // Indentation is significant only in the indented syntax, and not inside a
        // `(...)` group — there newlines are ignored (multi-line param/arg lists),
        // just like in SCSS.
        let indent_sensitive = self.syntax == Syntax::Sass && self.state.paren_depth == 0;
        // Sass can dedent more than one level at a time,
        // so we need to produce a dedent token for each level.
        if indent_sensitive
            && self.state.indents.last().is_some_and(|last| *last > self.state.current_indent)
            && let Some((i, _)) = self.state.chars.peek()
        {
            self.state.indents.pop();
            return Some(TokenWithSpan {
                token: Token::Dedent(Dedent {}),
                span: Span { start: *i, end: *i },
            });
        }
        let mut indent = None;
        loop {
            match self.state.chars.peek() {
                Some((_, c)) if c.is_ascii_whitespace() => {
                    if indent_sensitive {
                        indent = self.scan_indent();
                    } else {
                        self.skip_ws();
                    }
                }
                Some((_, '/')) => {
                    let mut chars = self.state.chars.clone();
                    chars.next();
                    match chars.next() {
                        Some((_, '*')) => self.scan_block_comment(),
                        Some((_, '/')) if self.syntax != Syntax::Css => self.scan_line_comment(),
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        indent
    }

    fn skip_ws(&mut self) {
        while let Some((_, c)) = self.state.chars.peek() {
            if c.is_ascii_whitespace() {
                self.state.chars.next();
            } else {
                break;
            }
        }
    }

    fn scan_indent(&mut self) -> Option<TokenWithSpan<'a>> {
        debug_assert_eq!(self.syntax, Syntax::Sass);
        let mut start = None;
        while let Some((i, c)) = self.state.chars.peek() {
            if c.is_ascii_whitespace() {
                let (i, c) = self.state.chars.next()?;
                // `\n`, a lone `\r` (old Mac), `\r\n`, and `\f` are all line
                // boundaries, matching dart-sass.
                if c == '\n' || c == '\r' || c == '\x0C' {
                    start = Some(i + 1);
                }
            } else {
                return start.map(|start| {
                    let end = *i;
                    let len = (end - start) as u16;
                    let span = Span { start, end };
                    self.state.current_indent = len;
                    match len.cmp(&self.state.indents.last().copied().unwrap_or_default()) {
                        Ordering::Greater => {
                            self.state.indents.push(len);
                            TokenWithSpan { token: Token::Indent(Indent {}), span }
                        }
                        Ordering::Less => {
                            self.state.indents.pop();
                            TokenWithSpan { token: Token::Dedent(Dedent {}), span }
                        }
                        Ordering::Equal => {
                            TokenWithSpan { token: Token::Linebreak(Linebreak {}), span }
                        }
                    }
                });
            }
        }

        let offset = self.current_offset();
        Some(TokenWithSpan { token: Token::Eof(Eof {}), span: Span { start: offset, end: offset } })
    }

    fn scan_block_comment(&mut self) {
        let (start, c) = self.state.chars.next().unwrap();
        debug_assert_eq!(c, '/');
        self.state.chars.next();

        let content_end;
        let end;
        loop {
            match self.state.chars.next() {
                Some((_, '*')) => {
                    if let Some((i, '/')) = self.state.chars.peek() {
                        content_end = i - 1;
                        end = i + 1;
                        self.state.chars.next();
                        break;
                    }
                }
                Some(..) => {}
                None => {
                    content_end = self.source.len();
                    end = content_end;
                    break;
                }
            }
        }

        if let Some(comments) = &mut self.comments {
            let content = unsafe { self.source.get_unchecked(start + 2..content_end) };
            comments.push(Comment { content, kind: CommentKind::Block, span: Span { start, end } });
        }
    }

    fn scan_line_comment(&mut self) {
        let (start, c) = self.state.chars.next().unwrap();
        debug_assert_eq!(c, '/');
        self.state.chars.next();

        let end;
        loop {
            match self.state.chars.peek() {
                Some((_, '\r')) => {
                    self.state.chars.next();
                    if let Some((i, '\n')) = self.state.chars.peek() {
                        end = i - 1;
                        if self.syntax != Syntax::Sass {
                            self.state.chars.next();
                        }
                        break;
                    }
                }
                Some((i, '\n')) => {
                    end = *i;
                    if self.syntax != Syntax::Sass {
                        self.state.chars.next();
                    }
                    break;
                }
                Some(..) => {
                    self.state.chars.next();
                }
                None => {
                    end = self.source.len();
                    break;
                }
            }
        }

        // In the indented syntax, lines indented deeper than the line the
        // comment started on continue the comment — but only for a comment
        // that starts its line (a statement-level comment); one trailing
        // after code is plain whitespace with no children.
        let starts_line = || {
            let bytes = self.source.as_bytes();
            let mut i = start;
            while i > 0 && matches!(bytes[i - 1], b' ' | b'\t') {
                i -= 1;
            }
            i == 0 || matches!(bytes[i - 1], b'\n' | b'\r' | b'\x0C')
        };
        let end = if self.syntax == Syntax::Sass && self.state.paren_depth == 0 && starts_line() {
            self.consume_sass_comment_continuation(end)
        } else {
            end
        };

        if let Some(comments) = &mut self.comments {
            let content = unsafe { self.source.get_unchecked(start + 2..end) };
            comments.push(Comment { content, kind: CommentKind::Line, span: Span { start, end } });
        }
    }

    /// Consume indented-syntax comment continuation lines (any line indented
    /// deeper than the line the comment started on), returning the new
    /// comment end offset. The terminating newline is left unconsumed so the
    /// usual indentation scanning still runs.
    fn consume_sass_comment_continuation(&mut self, mut end: usize) -> usize {
        let base = self.state.current_indent;
        loop {
            let mut probe = self.state.chars.clone();
            // step over the line break(s) and measure the next line's indent
            let mut saw_newline = false;
            let mut indent: u16 = 0;
            let mut content_start = None;
            for (i, c) in probe.by_ref() {
                match c {
                    '\n' | '\r' | '\x0C' => {
                        saw_newline = true;
                        indent = 0;
                    }
                    ' ' | '\t' => indent += 1,
                    _ => {
                        content_start = Some(i);
                        break;
                    }
                }
            }
            let Some(content_start) = content_start else { return end };
            if !saw_newline || indent <= base {
                return end;
            }
            // deeper content: the line belongs to the comment — consume it
            while let Some((i, c)) = self.state.chars.peek() {
                if matches!(c, '\n' | '\r' | '\x0C') {
                    if *i >= content_start {
                        break;
                    }
                    self.state.chars.next();
                } else {
                    end = i + c.len_utf8();
                    self.state.chars.next();
                }
            }
        }
    }

    pub(crate) fn scan_ident_sequence(
        &mut self,
        allow_leading_digit: bool,
    ) -> PResult<(Ident<'a>, Span)> {
        let start;
        let end;
        let mut escaped = false;
        match self.state.chars.peek() {
            Some((i, c)) if c.is_ascii_alphabetic() || *c == '_' || !c.is_ascii() => {
                start = *i;
                self.state.chars.next();
            }
            // Less variable names may start with a digit (`@3`, `@{3}`); CSS idents may not.
            Some((i, c)) if allow_leading_digit && c.is_ascii_digit() => {
                start = *i;
                self.state.chars.next();
            }
            Some((i, '-')) => {
                start = *i;
                self.state.chars.next();
                if let Some((_, c)) = self.state.chars.next() {
                    debug_assert!(is_start_of_ident(c));
                } else {
                    return Err(self.build_eof_error());
                }
            }
            Some((i, '\\')) => {
                escaped = true;
                start = *i;
                self.scan_escape(/* backslash_consumed */ false)?;
            }
            _ => unreachable!(),
        }

        loop {
            match self.state.chars.peek() {
                Some((_, c))
                    if c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || !c.is_ascii() =>
                {
                    self.state.chars.next();
                }
                Some((_, '\\')) => {
                    escaped = true;
                    self.scan_escape(/* backslash_consumed */ false)?;
                }
                Some((i, _)) => {
                    end = *i;
                    break;
                }
                None => {
                    end = self.source.len();
                    break;
                }
            }
        }

        debug_assert!(start < end);
        let raw = unsafe { self.source.get_unchecked(start..end) };
        Ok((Ident { raw, escaped }, Span { start, end }))
    }

    fn scan_escape(&mut self, backslash_consumed: bool) -> PResult<usize> {
        if !backslash_consumed {
            self.state.chars.next(); // consume `\\`
        }
        match self.state.chars.next() {
            Some((i, c)) if c.is_ascii_hexdigit() => {
                let mut count: usize = 1;
                let mut end = i + 1;
                while let Some((i, c)) = self.state.chars.peek() {
                    if c.is_ascii_hexdigit() && count < 6 {
                        count += 1;
                        self.state.chars.next();
                    } else {
                        // according to https://www.w3.org/TR/css-syntax-3/#hex-digit,
                        // consume a whitespace
                        if c.is_ascii_whitespace() {
                            end = i + 1;
                            self.state.chars.next();
                        } else {
                            end = *i;
                        }
                        break;
                    }
                }
                Ok(end)
            }
            Some((i, c)) => Ok(i + c.len_utf8()),
            None => Err(self.build_eof_error()),
        }
    }

    fn scan_number(&mut self) -> PResult<(Number<'a>, Span)> {
        let start;
        let mut end;

        let is_start_with_dot;
        match self.state.chars.next() {
            Some((i, c)) if c.is_ascii_digit() => {
                start = i;
                is_start_with_dot = false;
            }
            Some((i, '+' | '-')) => {
                start = i;
                is_start_with_dot = matches!(self.state.chars.next(), Some((_, '.')));
            }
            Some((i, '.')) => {
                start = i;
                is_start_with_dot = true;
            }
            _ => unreachable!(),
        }

        loop {
            match self.state.chars.peek() {
                Some((_, c)) if c.is_ascii_digit() => {
                    self.state.chars.next();
                }
                Some((i, _)) => {
                    end = *i;
                    break;
                }
                None => {
                    end = self.source.len();
                    break;
                }
            }
        }
        if !is_start_with_dot {
            let chars = self.state.chars.clone();
            match self.state.chars.peek() {
                // next token can be a `DotDotDot` token
                Some((_, '.')) if !matches!(chars.clone().nth(1), Some((_, '.'))) => {
                    // bump '.'
                    self.state.chars.next();
                    loop {
                        match self.state.chars.peek() {
                            Some((_, c)) if c.is_ascii_digit() => {
                                self.state.chars.next();
                            }
                            Some((i, _)) => {
                                end = *i;
                                break;
                            }
                            None => {
                                end = self.source.len();
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        match self.peek_two_chars() {
            Some((_, 'e' | 'E', second))
                if second == '-' || second == '+' || second.is_ascii_digit() =>
            {
                self.state.chars.next();

                if let Some((_, '-' | '+')) = self.state.chars.peek() {
                    self.state.chars.next();
                }

                loop {
                    match self.state.chars.clone().peek() {
                        Some((_, c)) if c.is_ascii_digit() => {
                            self.state.chars.next();
                        }
                        Some((i, _)) => {
                            end = *i;
                            break;
                        }
                        None => {
                            end = self.source.len();
                            break;
                        }
                    }
                }
            }
            _ => {}
        }

        debug_assert!(start < end);
        let raw = unsafe { self.source.get_unchecked(start..end) };
        Ok((Number { raw }, Span { start, end }))
    }

    fn scan_dimension_or_percentage(
        &mut self,
        number: Number<'a>,
        span: Span,
    ) -> PResult<TokenWithSpan<'a>> {
        let mut chars = self.state.chars.clone();
        match (chars.next(), chars.next()) {
            (Some((_, '-')), Some((_, c))) if is_start_of_ident(c) => {
                self.scan_dimension(number, span)
            }
            (Some((_, c)), ..) if c != '-' && is_start_of_ident(c) => {
                self.scan_dimension(number, span)
            }
            (Some((_, '%')), ..) => self.scan_percentage(number, span),
            _ => Ok(TokenWithSpan { token: Token::Number(number), span }),
        }
    }

    fn scan_dimension(
        &mut self,
        value: Number<'a>,
        value_span: Span,
    ) -> PResult<TokenWithSpan<'a>> {
        let (unit, unit_span) = self.scan_ident_sequence(false)?;
        Ok(TokenWithSpan {
            token: Token::Dimension(Dimension { value, unit }),
            span: Span { start: value_span.start, end: unit_span.end },
        })
    }

    fn scan_percentage(&mut self, value: Number<'a>, span: Span) -> PResult<TokenWithSpan<'a>> {
        self.state.chars.next();
        Ok(TokenWithSpan {
            token: Token::Percentage(Percentage { value }),
            span: Span { start: span.start, end: span.end + 1 },
        })
    }

    pub(crate) fn scan_string_only(&mut self) -> PResult<(Str<'a>, Span)> {
        let (start, quote) = match self.state.chars.next() {
            Some((index, c @ '\'' | c @ '"')) => (index, c),
            Some((index, _)) => {
                return Err(Error {
                    kind: ErrorKind::ExpectString,
                    span: Span { start: index, end: index + 1 },
                });
            }
            None => return Err(self.build_eof_error()),
        };

        let end;
        let mut escaped = false;
        loop {
            match self.state.chars.next() {
                Some((_, '\\')) => {
                    escaped = true;
                    self.scan_escape(/* backslash_consumed */ true)?;
                }
                Some((i, c)) if c == quote => {
                    end = i + 1;
                    break;
                }
                Some((end, '\n')) => {
                    return Err(Error {
                        kind: ErrorKind::UnterminatedString,
                        span: Span { start, end },
                    });
                }
                Some(..) => {}
                None => {
                    return Err(Error {
                        kind: ErrorKind::UnterminatedString,
                        span: Span { start, end: self.source.len() },
                    });
                }
            }
        }

        debug_assert!(start + 1 < end);
        let raw = unsafe { self.source.get_unchecked(start..end) };
        Ok((Str { raw, escaped }, Span { start, end }))
    }

    fn scan_string_or_template(&mut self) -> PResult<TokenWithSpan<'a>> {
        // '\'' or '"' is checked (but not consumed) before
        let (start, quote) = self.state.chars.next().unwrap();

        let end;
        let mut escaped = false;
        loop {
            match self.state.chars.next() {
                Some((_, '\\')) => {
                    escaped = true;
                    self.scan_escape(/* backslash_consumed */ true)?;
                }
                Some((i, c)) if c == quote => {
                    end = i + 1;
                    break;
                }
                Some((end, c @ '#' | c @ '@' | c @ '$'))
                    if self.is_start_of_interpolation_in_str_template(c) =>
                {
                    let raw = unsafe { self.source.get_unchecked(start..end) };
                    let span = Span { start, end };
                    return Ok(TokenWithSpan {
                        token: Token::StrTemplate(StrTemplate {
                            raw,
                            escaped,
                            head: true,
                            tail: false,
                        }),
                        span,
                    });
                }
                Some((end, '\n')) => {
                    // CSS Syntax: an unterminated string is a
                    // `<bad-string-token>` (parse error, not a lexer failure);
                    // the dialects' reference compilers reject it outright.
                    if self.syntax == Syntax::Css {
                        let raw = unsafe { self.source.get_unchecked(start..end) };
                        return Ok(TokenWithSpan {
                            token: Token::BadStr(BadStr { raw }),
                            span: Span { start, end },
                        });
                    }
                    return Err(Error {
                        kind: ErrorKind::UnterminatedString,
                        span: Span { start, end },
                    });
                }
                Some(..) => {}
                None => {
                    let end = self.source.len();
                    if self.syntax == Syntax::Css {
                        let raw = unsafe { self.source.get_unchecked(start..end) };
                        return Ok(TokenWithSpan {
                            token: Token::BadStr(BadStr { raw }),
                            span: Span { start, end },
                        });
                    }
                    return Err(Error {
                        kind: ErrorKind::UnterminatedString,
                        span: Span { start, end },
                    });
                }
            }
        }

        debug_assert!(start + 1 < end);
        let raw = unsafe { self.source.get_unchecked(start..end) };
        Ok(TokenWithSpan { token: Token::Str(Str { raw, escaped }), span: Span { start, end } })
    }

    pub(crate) fn scan_string_template(&mut self, quote: char) -> PResult<(StrTemplate<'a>, Span)> {
        let start = self.current_offset();
        let end;
        let mut escaped = false;
        loop {
            match self.state.chars.next() {
                Some((i, '\n')) => {
                    return Err(Error {
                        kind: ErrorKind::UnexpectedLinebreak,
                        span: Span { start: i, end: i + 1 },
                    });
                }
                Some((_, '\\')) => {
                    escaped = true;
                    self.scan_escape(/* backslash_consumed */ true)?;
                }
                Some((i, c)) if c == quote => {
                    end = i + c.len_utf8();
                    debug_assert!(start < end);

                    let raw = unsafe { self.source.get_unchecked(start..i + 1) };
                    let span = Span { start, end };
                    return Ok((StrTemplate { raw, escaped, head: false, tail: true }, span));
                }
                Some((end, c @ '#' | c @ '@' | c @ '$'))
                    if self.is_start_of_interpolation_in_str_template(c) =>
                {
                    let raw = unsafe { self.source.get_unchecked(start..end) };
                    let span = Span { start, end };
                    return Ok((StrTemplate { raw, escaped, head: false, tail: false }, span));
                }
                Some(..) => {}
                None => return Err(self.build_eof_error()),
            }
        }
    }

    fn is_start_of_interpolation_in_str_template(&mut self, c: char) -> bool {
        match self.syntax {
            Syntax::Css => false,
            Syntax::Scss | Syntax::Sass => {
                c == '#' && matches!(self.state.chars.peek(), Some((_, '{')))
            }
            Syntax::Less => {
                // Less interpolation names may start with a digit (`@{3}`), like `@3`.
                (c == '@' || c == '$')
                    && matches!(self.peek_two_chars(), Some((_, '{', second)) if is_start_of_ident(second) || second.is_ascii_digit())
            }
        }
    }

    fn scan_ident(&mut self) -> PResult<TokenWithSpan<'a>> {
        self.scan_ident_sequence(false)
            .map(|(ident, span)| TokenWithSpan { token: Token::Ident(ident), span })
    }

    pub(crate) fn scan_ident_template(&mut self) -> PResult<Option<(Ident<'a>, Span)>> {
        let start = self.current_offset();
        let mut escaped = false;

        let end;
        loop {
            match self.state.chars.peek() {
                Some((_, c))
                    if c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || !c.is_ascii() =>
                {
                    self.state.chars.next();
                }
                Some((_, '\\')) => {
                    escaped = true;
                    self.scan_escape(/* backslash_consumed */ false)?;
                }
                Some((i, _)) => {
                    end = *i;
                    break;
                }
                None => {
                    end = self.source.len();
                    break;
                }
            }
        }
        if end > start {
            let raw = unsafe { self.source.get_unchecked(start..end) };
            Ok(Some((Ident { escaped, raw }, Span { start, end })))
        } else {
            Ok(None)
        }
    }

    fn scan_sass_single_hyphen_as_ident(&mut self) -> PResult<TokenWithSpan<'a>> {
        debug_assert!(matches!(self.syntax, Syntax::Scss | Syntax::Sass));
        match self.state.chars.next() {
            Some((start, c)) => {
                debug_assert_eq!(c, '-');
                Ok(TokenWithSpan {
                    token: Token::Ident(Ident { escaped: false, raw: "-" }),
                    span: Span { start, end: start + 1 },
                })
            }
            None => Err(self.build_eof_error()),
        }
    }

    pub(crate) fn scan_url_raw_or_template(&mut self) -> PResult<TokenWithSpan<'a>> {
        self.skip_ws();
        let start = self.current_offset();
        let end;
        let mut escaped = false;
        loop {
            match self.state.chars.next() {
                Some((_, '\\')) => {
                    escaped = true;
                    self.scan_escape(/* backslash_consumed */ true)?;
                }
                Some((i, ')')) => {
                    // the matching `(` was consumed as an LParen token
                    self.state.paren_depth = self.state.paren_depth.saturating_sub(1);
                    end = i;
                    break;
                }
                Some((end, '#')) if self.is_start_of_interpolation_in_url_template() => {
                    let raw = unsafe { self.source.get_unchecked(start..end) };
                    let span = Span { start, end };
                    return Ok(TokenWithSpan {
                        token: Token::UrlTemplate(UrlTemplate { raw, escaped, tail: false }),
                        span,
                    });
                }
                Some((i, c)) if c.is_ascii_whitespace() => {
                    self.skip_ws();
                    match self.state.chars.next() {
                        Some((_, ')')) => {
                            self.state.paren_depth = self.state.paren_depth.saturating_sub(1);
                            end = i;
                            break;
                        }
                        Some((i, c)) => {
                            return Err(Error {
                                kind: ErrorKind::InvalidUrl,
                                span: Span { start: i, end: i + c.len_utf8() },
                            });
                        }
                        None => return Err(self.build_eof_error()),
                    }
                }
                Some((i, '(' | '"' | '\'')) => {
                    return Err(Error {
                        kind: ErrorKind::InvalidUrl,
                        span: Span { start: i, end: i + 1 },
                    });
                }
                Some(..) => {}
                None => return Err(self.build_eof_error()),
            }
        }

        debug_assert!(start <= end);
        let raw = unsafe { self.source.get_unchecked(start..end) };
        let span = Span { start, end };
        Ok(TokenWithSpan { token: Token::UrlRaw(UrlRaw { raw, escaped }), span })
    }

    pub(crate) fn scan_url_template(&mut self) -> PResult<(UrlTemplate<'a>, Span)> {
        let start = self.current_offset();
        let mut escaped = false;
        loop {
            match self.state.chars.next() {
                Some((_, '\\')) => {
                    escaped = true;
                    self.scan_escape(/* backslash_consumed */ true)?;
                }
                Some((end, ')')) => {
                    debug_assert!(start <= end);

                    self.state.paren_depth = self.state.paren_depth.saturating_sub(1);
                    let raw = unsafe { self.source.get_unchecked(start..end) };
                    let span = Span { start, end };
                    return Ok((UrlTemplate { raw, escaped, tail: true }, span));
                }
                Some((end, '#')) if self.is_start_of_interpolation_in_url_template() => {
                    let raw = unsafe { self.source.get_unchecked(start..end) };
                    let span = Span { start, end };
                    return Ok((UrlTemplate { raw, escaped, tail: false }, span));
                }
                Some((end, c)) if c.is_ascii_whitespace() => {
                    self.skip_ws();
                    match self.state.chars.next() {
                        Some((_, ')')) => {
                            self.state.paren_depth = self.state.paren_depth.saturating_sub(1);
                            return Ok((
                                UrlTemplate {
                                    raw: unsafe { self.source.get_unchecked(start..end) },
                                    escaped,
                                    tail: true,
                                },
                                Span { start, end },
                            ));
                        }
                        Some((i, c)) => {
                            return Err(Error {
                                kind: ErrorKind::InvalidUrl,
                                span: Span { start: i, end: i + c.len_utf8() },
                            });
                        }
                        None => return Err(self.build_eof_error()),
                    }
                }
                Some((i, '(' | '"' | '\'')) => {
                    return Err(Error {
                        kind: ErrorKind::InvalidUrl,
                        span: Span { start: i, end: i + 1 },
                    });
                }
                Some(..) => {}
                None => return Err(self.build_eof_error()),
            }
        }
    }

    fn is_start_of_interpolation_in_url_template(&mut self) -> bool {
        match self.syntax {
            Syntax::Css | Syntax::Less => false,
            Syntax::Scss | Syntax::Sass => matches!(self.state.chars.peek(), Some((_, '{'))),
        }
    }

    fn scan_hash(&mut self) -> PResult<TokenWithSpan<'a>> {
        let (start, c) = self.state.chars.next().unwrap();
        debug_assert_eq!(c, '#');

        let end;
        let mut escaped = false;
        match self.state.chars.next() {
            Some((_, c)) if c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii() => {}
            Some((_, '\\')) => {
                escaped = true;
                self.scan_escape(/* backslash_consumed */ true)?;
            }
            Some((i, _)) => {
                return Err(Error {
                    kind: ErrorKind::InvalidHash,
                    span: Span { start: i, end: i + c.len_utf8() },
                });
            }
            None => {
                return Err(self.build_eof_error());
            }
        }
        loop {
            match self.state.chars.peek() {
                Some((_, c))
                    if c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || !c.is_ascii() =>
                {
                    self.state.chars.next();
                }
                Some((_, '\\')) => {
                    escaped = true;
                    self.scan_escape(/* backslash_consumed */ false)?;
                }
                Some((i, _)) => {
                    end = *i;
                    break;
                }
                None => {
                    end = self.source.len();
                    break;
                }
            }
        }

        debug_assert!(end > start + 1);
        let raw = unsafe { self.source.get_unchecked(start + 1..end) };
        Ok(TokenWithSpan { token: Token::Hash(Hash { escaped, raw }), span: Span { start, end } })
    }

    fn scan_dollar_var(&mut self) -> PResult<TokenWithSpan<'a>> {
        let (start, c) = self.state.chars.next().expect("expect char `$`");
        debug_assert_eq!(c, '$');
        let (ident, span) = self.scan_ident_sequence(false)?;
        Ok(TokenWithSpan {
            token: Token::DollarVar(DollarVar { ident }),
            span: Span { start, end: span.end },
        })
    }

    fn scan_less_lbrace_var(&mut self) -> PResult<TokenWithSpan<'a>> {
        let (start, first_char) = self.state.chars.next().expect("expect char `@` or `$`");
        debug_assert!(matches!(first_char, '@' | '$'));
        let (_, c) = self.state.chars.next().expect("expect char `{`");
        debug_assert_eq!(c, '{');

        // Less allows digit-led variable names, so `@{3}` interpolates `@3`.
        let (ident, _) = self.scan_ident_sequence(true)?;
        match self.state.chars.next() {
            Some((i, '}')) => {
                let span = Span { start, end: i + 1 };
                if first_char == '@' {
                    Ok(TokenWithSpan { token: Token::AtLBraceVar(AtLBraceVar { ident }), span })
                } else {
                    Ok(TokenWithSpan {
                        token: Token::DollarLBraceVar(DollarLBraceVar { ident }),
                        span,
                    })
                }
            }
            Some((i, c)) => Err(Error {
                kind: ErrorKind::ExpectRightBraceForLessVar,
                span: Span { start: i, end: i + c.len_utf8() },
            }),
            None => Err(self.build_eof_error()),
        }
    }

    fn scan_at_keyword(&mut self) -> PResult<TokenWithSpan<'a>> {
        let (start, c) = self.state.chars.next().expect("expect char `@`");
        debug_assert_eq!(c, '@');

        // Less allows digit-led variable names like `@3`.
        let (ident, span) = self.scan_ident_sequence(self.syntax == Syntax::Less)?;
        Ok(TokenWithSpan {
            token: Token::AtKeyword(AtKeyword { ident }),
            span: Span { start, end: span.end },
        })
    }

    /// Scan a backtick-delimited template placeholder `` `<prefix><digits>` `` whose
    /// opening backtick is at the current position. Emits a [`Token::Placeholder`].
    /// A backtick that doesn't form a valid placeholder is invalid (backtick is
    /// not valid SCSS) and errors like any unknown token.
    fn scan_template_placeholder(&mut self) -> PResult<TokenWithSpan<'a>> {
        let start = self.current_offset();
        if let Some((placeholder, end)) = self.match_placeholder(start) {
            // Affixes and digits are ASCII, so `end` lands on a char boundary.
            while self.current_offset() < end {
                self.state.chars.next();
            }
            Ok(TokenWithSpan { token: Token::Placeholder(placeholder), span: Span { start, end } })
        } else {
            self.state.chars.next(); // consume the stray backtick
            Err(Error { kind: ErrorKind::UnknownToken, span: Span { start, end: start + 1 } })
        }
    }

    /// Match the placeholder shape `` `<prefix><digits>` `` whose opening backtick
    /// is at byte offset `at`. Returns the parsed token and its end offset (past
    /// the closing backtick and any glued suffix), or `None` when the option is
    /// unset or the shape doesn't match. An index that overflows `u32` doesn't
    /// match (so the caller errors) instead of panicking. Does not advance.
    fn match_placeholder(&self, at: usize) -> Option<(Placeholder<'a>, usize)> {
        // Bind `source` as `&'a str` so the suffix slice has lifetime `'a`
        // (independent of `&self`), letting callers mutate the tokenizer while
        // holding the returned token.
        let source: &'a str = self.source;
        let ph = self.template_placeholder?;
        // `at` is the opening backtick.
        let after_open = source[at + 1..].strip_prefix(ph.prefix)?;
        let digits_len = after_open.bytes().take_while(u8::is_ascii_digit).count();
        if digits_len == 0 || !after_open[digits_len..].starts_with('`') {
            return None;
        }
        let index = after_open[..digits_len].parse::<u32>().ok()?;
        // Past the closing backtick (`+ 1`).
        let close_end = at + 1 + ph.prefix.len() + digits_len + 1;
        // A directly-glued ident-continuation run is the placeholder's literal
        // suffix (`` `PLACEHOLDER-0`px `` -> index 0, suffix "px"), mirroring
        // `#{$x}px` being a single identifier. A whitespace or delimiter ends it.
        // Bytes >= 0x80 are non-ASCII ident chars (whole UTF-8 sequences), so the
        // run always ends on a char boundary.
        let suffix_len = source[close_end..]
            .bytes()
            .take_while(|&b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80)
            .count();
        let end = close_end + suffix_len;
        Some((Placeholder { index, suffix: &source[close_end..end] }, end))
    }

    /// If a placeholder begins exactly at the current position, scan and return
    /// it (advancing past it); otherwise leave the tokenizer untouched and
    /// return `None`. Used where a placeholder must be detected without first
    /// skipping whitespace (e.g. a class selector name), so callers can't rely
    /// on the main dispatch having already emitted the token.
    pub(crate) fn scan_placeholder(&mut self) -> Option<TokenWithSpan<'a>> {
        let start = self.current_offset();
        if !self.source[start..].starts_with('`') {
            return None;
        }
        let (placeholder, end) = self.match_placeholder(start)?;
        while self.current_offset() < end {
            self.state.chars.next();
        }
        Some(TokenWithSpan { token: Token::Placeholder(placeholder), span: Span { start, end } })
    }

    fn scan_backtick_code(&mut self) -> PResult<TokenWithSpan<'a>> {
        debug_assert!(self.syntax == Syntax::Less);

        // '`' is checked (but not consumed) before
        let (start, _) = self.state.chars.next().expect("expect char ```");

        let end;
        loop {
            match self.state.chars.next() {
                Some((i, '`')) => {
                    end = i + 1;
                    break;
                }
                Some(..) => {}
                None => {
                    return Err(self.build_eof_error());
                }
            }
        }

        debug_assert!(start + 1 < end);
        let raw = unsafe { self.source.get_unchecked(start..end) };
        Ok(TokenWithSpan {
            token: Token::BacktickCode(BacktickCode { raw }),
            span: Span { start, end },
        })
    }

    fn scan_punc(&mut self) -> PResult<TokenWithSpan<'a>> {
        match self.state.chars.next() {
            Some((start, '.')) => {
                if self.syntax != Syntax::Css
                    && matches!(
                        {
                            let mut chars = self.state.chars.clone();
                            (chars.next(), chars.next())
                        },
                        (Some((_, '.')), Some((_, '.')))
                    )
                {
                    self.state.chars.next();
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::DotDotDot(DotDotDot {}),
                        span: Span { start, end: start + 3 },
                    })
                } else {
                    Ok(TokenWithSpan {
                        token: Token::Dot(Dot {}),
                        span: Span { start, end: start + 1 },
                    })
                }
            }
            Some((start, ':')) => match self.state.chars.peek() {
                Some((_, ':')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::ColonColon(ColonColon {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Colon(Colon {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '}')) => {
                // In the indented syntax `}` only ever closes `#{`; see the
                // HashLBrace arm below.
                if self.syntax == Syntax::Sass {
                    self.state.paren_depth = self.state.paren_depth.saturating_sub(1);
                }
                Ok(TokenWithSpan {
                    token: Token::RBrace(RBrace {}),
                    span: Span { start, end: start + 1 },
                })
            }
            Some((start, '(')) => {
                self.state.paren_depth += 1;
                Ok(TokenWithSpan {
                    token: Token::LParen(LParen {}),
                    span: Span { start, end: start + 1 },
                })
            }
            Some((start, ')')) => {
                self.state.paren_depth = self.state.paren_depth.saturating_sub(1);
                Ok(TokenWithSpan {
                    token: Token::RParen(RParen {}),
                    span: Span { start, end: start + 1 },
                })
            }
            Some((start, '[')) => {
                // Like `(...)`: newlines inside `[...]` are insignificant in
                // the indented syntax (multi-line attribute selectors and
                // bracketed lists).
                self.state.paren_depth += 1;
                Ok(TokenWithSpan {
                    token: Token::LBracket(LBracket {}),
                    span: Span { start, end: start + 1 },
                })
            }
            Some((start, ']')) => {
                self.state.paren_depth = self.state.paren_depth.saturating_sub(1);
                Ok(TokenWithSpan {
                    token: Token::RBracket(RBracket {}),
                    span: Span { start, end: start + 1 },
                })
            }
            Some((start, '/')) => Ok(TokenWithSpan {
                token: Token::Solidus(Solidus {}),
                span: Span { start, end: start + 1 },
            }),
            Some((start, ',')) => Ok(TokenWithSpan {
                token: Token::Comma(Comma {}),
                span: Span { start, end: start + 1 },
            }),
            Some((start, ';')) => Ok(TokenWithSpan {
                token: Token::Semicolon(Semicolon {}),
                span: Span { start, end: start + 1 },
            }),
            Some((start, '>')) => match self.state.chars.peek() {
                Some((_, '=')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::GreaterThanEqual(GreaterThanEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::GreaterThan(GreaterThan {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '<')) => match self.state.chars.peek() {
                Some((_, '=')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::LessThanEqual(LessThanEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                Some((_, '!')) => {
                    let mut chars = self.state.chars.clone();
                    if {
                        chars.next();
                        matches!((chars.next(), chars.peek()), (Some((_, '-')), Some((_, '-'))))
                    } {
                        self.state.chars.next();
                        self.state.chars.next();
                        self.state.chars.next();
                        Ok(TokenWithSpan {
                            token: Token::Cdo(Cdo {}),
                            span: Span { start, end: start + 4 },
                        })
                    } else {
                        Ok(TokenWithSpan {
                            token: Token::LessThan(LessThan {}),
                            span: Span { start, end: start + 1 },
                        })
                    }
                }
                _ => Ok(TokenWithSpan {
                    token: Token::LessThan(LessThan {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '+')) => match self.state.chars.peek() {
                Some((_, '_')) if self.syntax == Syntax::Less => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::PlusUnderscore(PlusUnderscore {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Plus(Plus {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '=')) => match self.state.chars.peek() {
                Some((_, '=')) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::EqualEqual(EqualEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Equal(Equal {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '-')) => Ok(TokenWithSpan {
                token: Token::Minus(Minus {}),
                span: Span { start, end: start + 1 },
            }),
            Some((start, '~')) => match self.state.chars.peek() {
                Some((_, '=')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::TildeEqual(TildeEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Tilde(Tilde {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '&')) => Ok(TokenWithSpan {
                token: Token::Ampersand(Ampersand {}),
                span: Span { start, end: start + 1 },
            }),
            Some((start, '*')) => match self.state.chars.peek() {
                Some((_, '=')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::AsteriskEqual(AsteriskEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Asterisk(Asterisk {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '|')) => match self.state.chars.peek() {
                Some((_, '=')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::BarEqual(BarEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                Some((_, '|')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::BarBar(BarBar {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Bar(Bar {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '^')) => match self.state.chars.peek() {
                Some((_, '=')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::CaretEqual(CaretEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Unknown(Unknown {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '$')) => match self.state.chars.peek() {
                Some((_, '=')) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::DollarEqual(DollarEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Unknown(Unknown {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '!')) => match self.state.chars.peek() {
                Some((_, '=')) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                    self.state.chars.next();
                    Ok(TokenWithSpan {
                        token: Token::ExclamationEqual(ExclamationEqual {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::Exclamation(Exclamation {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '?')) => Ok(TokenWithSpan {
                token: Token::Question(Question {}),
                span: Span { start, end: start + 1 },
            }),
            Some((start, '#')) => match self.state.chars.peek() {
                Some((_, '{')) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                    self.state.chars.next();
                    // Newlines inside `#{...}` are insignificant in the
                    // indented syntax; the matching `}` closes it (see the
                    // RBrace arm above).
                    if self.syntax == Syntax::Sass {
                        self.state.paren_depth += 1;
                    }
                    Ok(TokenWithSpan {
                        token: Token::HashLBrace(HashLBrace {}),
                        span: Span { start, end: start + 2 },
                    })
                }
                _ => Ok(TokenWithSpan {
                    token: Token::NumberSign(NumberSign {}),
                    span: Span { start, end: start + 1 },
                }),
            },
            Some((start, '%')) => Ok(TokenWithSpan {
                token: Token::Percent(Percent {}),
                span: Span { start, end: start + 1 },
            }),
            Some((start, '@')) => {
                Ok(TokenWithSpan { token: Token::At(At {}), span: Span { start, end: start + 1 } })
            }
            Some((i, c)) if c.is_ascii_whitespace() => Err(Error {
                kind: ErrorKind::UnexpectedWhitespace,
                span: Span { start: i, end: i + 1 },
            }),
            // CSS Syntax: anything else is a <delim-token>, not a tokenizer
            // error. Typed grammar rules reject it where it doesn't belong;
            // raw component-value contexts preserve it.
            Some((i, c)) => Ok(TokenWithSpan {
                token: Token::Unknown(Unknown {}),
                span: Span { start: i, end: i + c.len_utf8() },
            }),
            None => {
                let offset = self.current_offset();
                Ok(TokenWithSpan {
                    token: Token::Eof(Eof {}),
                    span: Span { start: offset, end: offset },
                })
            }
        }
    }

    #[cold]
    fn scan_cdc(&mut self, start: usize) -> PResult<TokenWithSpan<'a>> {
        self.state.chars.next();
        self.state.chars.next();
        self.state.chars.next();
        Ok(TokenWithSpan { token: Token::Cdc(Cdc {}), span: Span { start, end: start + 3 } })
    }

    #[inline]
    pub(crate) fn is_start_of_ident(&mut self) -> bool {
        match self.state.chars.peek() {
            Some((_, c)) if is_start_of_ident(*c) => true,
            Some((_, '-')) => {
                let mut chars = self.state.chars.clone();
                chars.next();
                matches!(chars.peek(), Some((_, c)) if is_start_of_ident(*c))
            }
            _ => false,
        }
    }

    /// Whether the next code point is an ASCII digit — the start of a Less
    /// digit-led variable name (`@3`, `@{3}`).
    pub(crate) fn is_start_of_digit(&mut self) -> bool {
        matches!(self.state.chars.peek(), Some((_, c)) if c.is_ascii_digit())
    }

    pub(crate) fn is_start_of_url_string(&mut self) -> bool {
        self.skip_ws();
        matches!(self.state.chars.peek(), Some((_, '"' | '\'')))
    }
}

#[inline]
fn is_start_of_ident(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '-' || c == '_' || !c.is_ascii() || c == '\\'
}

/// Whether an identifier starts at byte offset `at` of `source`, using the
/// same dash lookahead as the tokenizer's dispatch (`-x` starts one, a lone
/// `-` doesn't).
pub(crate) fn ident_starts_at(source: &str, at: usize) -> bool {
    let mut chars = source.get(at..).unwrap_or_default().chars();
    match chars.next() {
        Some('-') => matches!(chars.next(), Some(c) if is_start_of_ident(c) && c != '-'),
        Some(c) => is_start_of_ident(c),
        None => false,
    }
}

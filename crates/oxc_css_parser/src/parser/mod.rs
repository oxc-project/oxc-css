use self::state::ParserState;
use crate::{
    ParserOptions,
    ast::{
        Dimension, Ident, InterpolableIdentStaticPart, InterpolableStrStaticPart,
        InterpolableUrlStaticPart, Str,
    },
    config::Syntax,
    error::{Error, PResult},
    tokenizer::{TokenWithSpan, Tokenizer, token},
    util,
};
pub use builder::ParserBuilder;
use oxc_allocator::{Allocator, Vec as ArenaVec};

mod at_rule;
mod builder;
mod convert;
mod less;
mod macros;
mod sass;
mod selector;
mod state;
mod stmt;
mod token_seq;
mod value;

pub trait Parse<'a>: Sized {
    fn parse(input: &mut Parser<'a>) -> PResult<Self>;
}

/// Create a parser with some source code, then parse it.
pub struct Parser<'a> {
    allocator: &'a Allocator,
    source: &'a str,
    syntax: Syntax,
    options: ParserOptions,
    tokenizer: Tokenizer<'a>,
    state: ParserState,
    recoverable_errors: Vec<Error>,
    cached_token: Option<TokenWithSpan<'a>>,
}

impl<'a> Parser<'a> {
    /// Create a parser with the given source code and specified syntax.
    /// If you need to control more options, please use [`ParserBuilder`].
    pub fn new(allocator: &'a Allocator, source: &'a str, syntax: Syntax) -> Self {
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        Parser {
            allocator,
            source,
            syntax,
            options: Default::default(),
            tokenizer: Tokenizer::new(allocator, source, syntax, None, false),
            state: Default::default(),
            recoverable_errors: vec![],
            cached_token: None,
        }
    }

    /// Start to parse.
    pub fn parse<T>(&mut self) -> PResult<T>
    where
        T: Parse<'a>,
    {
        T::parse(self)
    }

    /// Retrieve recoverable errors.
    #[inline]
    pub fn recoverable_errors(&self) -> &[Error] {
        &self.recoverable_errors
    }

    /// Retrieve collected comments.
    #[inline]
    pub fn comments(&self) -> &[crate::tokenizer::token::Comment<'a>] {
        self.tokenizer.comments()
    }

    #[inline]
    pub(crate) fn allocator(&self) -> &'a Allocator {
        self.allocator
    }

    #[inline]
    pub(crate) fn vec<T>(&self) -> ArenaVec<'a, T> {
        ArenaVec::new_in(self.allocator)
    }

    #[inline]
    pub(crate) fn vec_with_capacity<T>(&self, capacity: usize) -> ArenaVec<'a, T> {
        ArenaVec::with_capacity_in(capacity, self.allocator)
    }

    #[inline]
    pub(crate) fn ident(&self, token: token::Ident<'a>, span: crate::Span) -> Ident<'a> {
        Ident { name: self.ident_name(&token), raw: token.raw, span }
    }

    pub(crate) fn dimension(
        &self,
        token: token::Dimension<'a>,
        span: crate::Span,
    ) -> PResult<Dimension<'a>> {
        let value_span = crate::Span { start: span.start, end: span.start + token.value.raw.len() };
        let unit_span = crate::Span { start: span.start + token.value.raw.len(), end: span.end };
        let value = (token.value, value_span).try_into()?;
        let unit = self.ident(token.unit, unit_span);
        let kind = convert::dimension_kind(unit.name);
        Ok(Dimension { value, unit, kind, span })
    }

    #[inline]
    pub(crate) fn interpolable_ident_static_part(
        &self,
        token: token::Ident<'a>,
        span: crate::Span,
    ) -> InterpolableIdentStaticPart<'a> {
        InterpolableIdentStaticPart { value: self.ident_name(&token), raw: token.raw, span }
    }

    #[inline]
    pub(crate) fn str(&self, token: token::Str<'a>, span: crate::Span) -> Str<'a> {
        let raw_without_quotes = unsafe { token.raw.get_unchecked(1..token.raw.len() - 1) };
        let value = if token.escaped {
            util::handle_escape_in(raw_without_quotes, self.allocator)
        } else {
            raw_without_quotes
        };
        Str { value, raw: token.raw, span }
    }

    #[inline]
    pub(crate) fn interpolable_str_static_part(
        &self,
        token: token::StrTemplate<'a>,
        span: crate::Span,
    ) -> InterpolableStrStaticPart<'a> {
        let raw_without_quotes = if token.tail {
            unsafe { token.raw.get_unchecked(0..token.raw.len() - 1) }
        } else if token.head {
            unsafe { token.raw.get_unchecked(1..token.raw.len()) }
        } else {
            token.raw
        };
        let value = if token.escaped {
            util::handle_escape_in(raw_without_quotes, self.allocator)
        } else {
            raw_without_quotes
        };
        InterpolableStrStaticPart { value, raw: token.raw, span }
    }

    #[inline]
    pub(crate) fn interpolable_url_static_part(
        &self,
        token: token::UrlTemplate<'a>,
        span: crate::Span,
    ) -> InterpolableUrlStaticPart<'a> {
        let value = if token.escaped {
            util::handle_escape_in(token.raw, self.allocator)
        } else {
            token.raw
        };
        InterpolableUrlStaticPart { value, raw: token.raw, span }
    }

    #[inline]
    fn ident_name(&self, token: &token::Ident<'a>) -> &'a str {
        if token.escaped { util::handle_escape_in(token.raw, self.allocator) } else { token.raw }
    }

    fn try_parse<R, F: FnOnce(&mut Self) -> PResult<R>>(&mut self, f: F) -> PResult<R> {
        let tokenizer_state = self.tokenizer.state.clone();
        let comments_count = self.tokenizer.comments_count();
        let recoverable_errors_count = self.recoverable_errors.len();
        let cached_token = self.cached_token.clone();
        let result = f(self);
        if result.is_err() {
            self.tokenizer.state = tokenizer_state;
            self.tokenizer.truncate_comments(comments_count);
            self.recoverable_errors.truncate(recoverable_errors_count);
            self.cached_token = cached_token;
        }
        result
    }
}

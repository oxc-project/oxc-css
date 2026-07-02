use super::{Parser, state::QualifiedRuleContext};
use crate::{
    Parse, Syntax, arena_box, arena_vec,
    ast::*,
    bump, eat,
    error::{Error, ErrorKind, PResult},
    expect, peek,
    pos::{Span, Spanned},
    tokenizer::{Token, TokenWithSpan},
    util,
};

const PRECEDENCE_MULTIPLY: u8 = 2;
const PRECEDENCE_PLUS: u8 = 1;

/// Strip one leading `-vendor-` prefix (`-moz-calc` -> `calc`); returns the
/// name unchanged when there is none.
fn unvendored(name: &str) -> &str {
    name.strip_prefix('-').and_then(|rest| rest.split_once('-')).map_or(name, |(_, base)| base)
}

/// dart-sass "special functions" whose contents may be raw text rather than
/// values, but which are worth a typed parse first (so plain `element(#id)`
/// or `-webkit-calc(1px + 2px)` keep their structured AST): `element(...)`
/// and `type(...)`, plus `calc(...)`/`url(...)` under an unrecognized vendor
/// prefix. (`expression(...)` and `progid:...(...)` are always raw, and an
/// unvendored `calc`/`url` is parsed as a real calculation/URL.)
fn is_special_typed_or_raw_function(name: &str) -> bool {
    let base = unvendored(name);
    let vendored = base.len() != name.len();
    base.eq_ignore_ascii_case("element")
        || (!vendored && base.eq_ignore_ascii_case("type"))
        || (vendored && (base.eq_ignore_ascii_case("calc") || base.eq_ignore_ascii_case("url")))
}

impl<'a> Parser<'a> {
    pub(in crate::parser) fn parse_calc_expr(
        &mut self,
        allow_modulo: bool,
    ) -> PResult<ComponentValue<'a>> {
        self.parse_calc_expr_recursively(0, allow_modulo)
    }

    fn parse_calc_expr_recursively(
        &mut self,
        precedence: u8,
        allow_modulo: bool,
    ) -> PResult<ComponentValue<'a>> {
        let mut left = if precedence >= PRECEDENCE_MULTIPLY {
            if eat!(self, LParen).is_some() {
                let expr = self.parse_calc_expr(allow_modulo)?;
                expect!(self, RParen);
                expr
            } else if self.syntax == Syntax::Less {
                if matches!(peek!(self).token, Token::Minus(..)) {
                    ComponentValue::LessNegativeValue(self.parse()?)
                } else {
                    self.parse_component_value_atom()?
                }
            } else {
                self.parse_component_value_atom()?
            }
        } else {
            self.parse_calc_expr_recursively(precedence + 1, allow_modulo)?
        };

        loop {
            let operator = match &peek!(self).token {
                Token::Asterisk(..) if precedence == PRECEDENCE_MULTIPLY => {
                    CalcOperator { kind: CalcOperatorKind::Multiply, span: bump!(self).span }
                }
                Token::Solidus(..) if precedence == PRECEDENCE_MULTIPLY => {
                    CalcOperator { kind: CalcOperatorKind::Division, span: bump!(self).span }
                }
                // Sass modulo (`%`) shares multiplicative precedence, but only the
                // legacy SassScript `min`/`max` accept it (`allow_modulo`); true
                // calculations (`calc`, `clamp`, `sin`, ...) reject it, as does CSS.
                Token::Percent(..) if precedence == PRECEDENCE_MULTIPLY && allow_modulo => {
                    CalcOperator { kind: CalcOperatorKind::Modulo, span: bump!(self).span }
                }
                Token::Plus(..) if precedence == PRECEDENCE_PLUS => {
                    CalcOperator { kind: CalcOperatorKind::Plus, span: bump!(self).span }
                }
                Token::Minus(..) if precedence == PRECEDENCE_PLUS => {
                    CalcOperator { kind: CalcOperatorKind::Minus, span: bump!(self).span }
                }
                _ => break,
            };

            let right = self.parse_calc_expr_recursively(precedence + 1, allow_modulo)?;
            let span = Span { start: left.span().start, end: right.span().end };
            left = ComponentValue::Calc(Calc {
                left: arena_box!(self, left),
                op: operator,
                right: arena_box!(self, right),
                span,
            });
        }

        Ok(left)
    }

    pub(super) fn parse_component_value_atom(&mut self) -> PResult<ComponentValue<'a>> {
        let token_with_span = peek!(self);
        match &token_with_span.token {
            Token::Ident(token) => {
                if token.name().eq_ignore_ascii_case("url") {
                    match self.try_parse(Url::parse) {
                        Ok(url) => return Ok(ComponentValue::Url(arena_box!(self, url))),
                        Err(Error { kind: ErrorKind::TryParseError, .. }) => {}
                        Err(error) => {
                            // Not a `<url-token>` (quotes, parens, or raw
                            // whitespace inside). Reference compilers accept
                            // these as a function call with raw-ish contents
                            // (`url(fn("s"))`, multi-line data: URIs), so fall
                            // back to a function parse, keeping the original
                            // error if even that shape doesn't fit.
                            let (function_name, function_name_span) = expect!(self, Ident);
                            let function_name = self.ident(function_name, function_name_span);
                            return self
                                .parse_function_typed_or_raw(function_name)
                                .map(ComponentValue::Function)
                                .map_err(|_| error);
                        }
                    }
                }
                let ident = self.parse::<InterpolableIdent>()?;
                let ident_end = ident.span().end;
                match peek!(self) {
                    TokenWithSpan { token: Token::LParen(..), span } if span.start == ident_end => {
                        return match ident {
                            InterpolableIdent::Literal(ident)
                                if ident.name.eq_ignore_ascii_case("src") =>
                            {
                                self.parse_src_url(ident)
                                    .map(|url| ComponentValue::Url(arena_box!(self, url)))
                            }
                            InterpolableIdent::Literal(ident)
                                if unvendored(ident.name).eq_ignore_ascii_case("expression") =>
                            {
                                // IE `expression(...)` (any vendor prefix):
                                // contents are script, not CSS values.
                                self.parse_raw_function(ident).map(ComponentValue::Function)
                            }
                            InterpolableIdent::Literal(ident)
                                if is_special_typed_or_raw_function(ident.name) =>
                            {
                                self.parse_function_typed_or_raw(ident)
                                    .map(ComponentValue::Function)
                            }
                            ident => self.parse_function(ident).map(ComponentValue::Function),
                        };
                    }
                    // IE filter syntax `-c-progid:d.e(...)` — everything to
                    // the matching `)` is raw. (An unprefixed `progid:` at the
                    // start of a value takes the whole-value raw path in
                    // `Declaration::parse` instead.)
                    TokenWithSpan { token: Token::Colon(..), span }
                        if span.start == ident_end
                            && matches!(
                                &ident,
                                InterpolableIdent::Literal(id)
                                    if unvendored(id.name).eq_ignore_ascii_case("progid")
                            ) =>
                    {
                        if let InterpolableIdent::Literal(ident) = ident {
                            return self.parse_progid_function(ident).map(ComponentValue::Function);
                        }
                        unreachable!("guard matched a literal ident");
                    }
                    TokenWithSpan { token: Token::Dot(..), span }
                        if matches!(self.syntax, Syntax::Scss | Syntax::Sass)
                            && span.start == ident_end =>
                    {
                        if let InterpolableIdent::Literal(module) = ident {
                            let name = self.parse_sass_qualified_name(module)?;
                            return if let SassQualifiedName {
                                member: SassModuleMemberName::Ident(..),
                                ..
                            } = name
                            {
                                let (_, lparen_span) = expect!(self, LParen);
                                util::assert_no_ws_or_comment(&name.span, &lparen_span)?;
                                let args = self.parse_function_args()?;
                                let (_, Span { end, .. }) = expect!(self, RParen);
                                let span = Span { start: name.span.start, end };
                                Ok(ComponentValue::Function(Function {
                                    name: FunctionName::SassQualifiedName(arena_box!(self, name)),
                                    args,
                                    span,
                                }))
                            } else {
                                Ok(ComponentValue::SassQualifiedName(arena_box!(self, name)))
                            };
                        }
                    }
                    _ => {}
                }
                match ident {
                    InterpolableIdent::Literal(ident) if ident.raw.eq_ignore_ascii_case("u") => {
                        match peek!(self) {
                            TokenWithSpan { token: Token::Plus(..), span }
                                if span.start == ident_end =>
                            {
                                self.parse_unicode_range(ident).map(ComponentValue::UnicodeRange)
                            }
                            TokenWithSpan { token: Token::Number(token), span }
                                if token.raw.starts_with('+') && span.start == ident_end =>
                            {
                                self.parse_unicode_range(ident).map(ComponentValue::UnicodeRange)
                            }
                            TokenWithSpan { token: Token::Dimension(token), span }
                                if token.value.raw.starts_with('+') && span.start == ident_end =>
                            {
                                self.parse_unicode_range(ident).map(ComponentValue::UnicodeRange)
                            }
                            _ => Ok(ComponentValue::InterpolableIdent(InterpolableIdent::Literal(
                                ident,
                            ))),
                        }
                    }
                    _ => Ok(ComponentValue::InterpolableIdent(ident)),
                }
            }
            Token::Solidus(..) | Token::Comma(..) => self.parse().map(ComponentValue::Delimiter),
            Token::Number(..) => self.parse().map(ComponentValue::Number),
            Token::Dimension(..) => self.parse().map(ComponentValue::Dimension),
            Token::Percentage(..) => self.parse().map(ComponentValue::Percentage),
            Token::Hash(..) => {
                if self.syntax == Syntax::Less {
                    self.parse_maybe_hex_color_or_less_mixin_call()
                } else {
                    self.parse().map(ComponentValue::HexColor)
                }
            }
            Token::Str(..) => {
                self.parse().map(InterpolableStr::Literal).map(ComponentValue::InterpolableStr)
            }
            Token::LBracket(..) => self.parse().map(ComponentValue::BracketBlock),
            Token::DollarVar(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                self.parse().map(ComponentValue::SassVariable)
            }
            Token::DollarVar(..)
                if self.syntax == Syntax::Css && self.options.allow_postcss_simple_vars =>
            {
                self.parse().map(ComponentValue::PostcssSimpleVar)
            }
            Token::LParen(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                match self.try_parse(SassParenthesizedExpression::parse) {
                    Ok(expr) => Ok(ComponentValue::SassParenthesizedExpression(expr)),
                    Err(err) => self.parse().map(ComponentValue::SassMap).map_err(|_| err),
                }
            }
            Token::HashLBrace(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                let ident = self.parse_sass_interpolated_ident()?;
                match peek!(self) {
                    TokenWithSpan { token: Token::LParen(..), span }
                        if span.start == ident.span().end =>
                    {
                        self.parse_function(ident).map(ComponentValue::Function)
                    }
                    _ => Ok(ComponentValue::InterpolableIdent(ident)),
                }
            }
            Token::StrTemplate(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => self
                .parse()
                .map(InterpolableStr::SassInterpolated)
                .map(ComponentValue::InterpolableStr),
            Token::Ampersand(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                self.parse().map(ComponentValue::SassParentSelector)
            }
            Token::LBrace(..)
                if self.syntax == Syntax::Scss
                    && matches!(
                        self.state.qualified_rule_ctx,
                        Some(QualifiedRuleContext::DeclarationValue)
                    ) =>
            {
                self.parse().map(ComponentValue::SassNestingDeclaration)
            }
            Token::Indent(..)
                if self.syntax == Syntax::Sass
                    && matches!(
                        self.state.qualified_rule_ctx,
                        Some(QualifiedRuleContext::DeclarationValue)
                    ) =>
            {
                self.parse().map(ComponentValue::SassNestingDeclaration)
            }
            Token::AtKeyword(..) if self.syntax == Syntax::Less => {
                self.parse_less_maybe_variable_or_with_lookups()
            }
            Token::Dot(..) if self.syntax == Syntax::Less => {
                self.parse_less_maybe_mixin_call_or_with_lookups()
            }
            Token::StrTemplate(..) if self.syntax == Syntax::Less => self
                .parse()
                .map(InterpolableStr::LessInterpolated)
                .map(ComponentValue::InterpolableStr),
            Token::At(..) if self.syntax == Syntax::Less => {
                self.parse().map(ComponentValue::LessVariableVariable)
            }
            Token::DollarVar(..) if self.syntax == Syntax::Less => {
                self.parse().map(ComponentValue::LessPropertyVariable)
            }
            Token::Tilde(..) if self.syntax == Syntax::Less => {
                if let Ok(list_function_call) = self.try_parse(Function::parse) {
                    Ok(ComponentValue::Function(list_function_call))
                } else if let Ok(less_escaped_str) = self.try_parse(LessEscapedStr::parse) {
                    Ok(ComponentValue::LessEscapedStr(less_escaped_str))
                } else {
                    self.parse().map(ComponentValue::LessJavaScriptSnippet)
                }
            }
            Token::Percent(..) if self.syntax == Syntax::Less => self
                .try_parse(Function::parse)
                .map(ComponentValue::Function)
                .or_else(|_| self.parse().map(ComponentValue::LessPercentKeyword)),
            Token::BacktickCode(..) if self.syntax == Syntax::Less => {
                self.parse().map(ComponentValue::LessJavaScriptSnippet)
            }
            Token::Placeholder(..) => {
                let (placeholder, span) = expect!(self, Placeholder);
                Ok(ComponentValue::Placeholder((placeholder, span).into()))
            }
            _ => Err(Error {
                kind: ErrorKind::ExpectComponentValue,
                span: token_with_span.span.clone(),
            }),
        }
    }

    pub(super) fn parse_dashed_ident(&mut self) -> PResult<InterpolableIdent<'a>> {
        let ident = self.parse()?;
        match &ident {
            InterpolableIdent::Literal(ident) if !ident.name.starts_with("--") => {
                self.recoverable_errors
                    .push(Error { kind: ErrorKind::ExpectDashedIdent, span: ident.span.clone() });
            }
            _ => {}
        }
        Ok(ident)
    }

    pub(super) fn parse_function(&mut self, name: InterpolableIdent<'a>) -> PResult<Function<'a>> {
        expect!(self, LParen);
        let args = if let Token::RParen(..) = &peek!(self).token {
            arena_vec!(self)
        } else {
            match &name {
                InterpolableIdent::Literal(ident)
                    if ident.name.eq_ignore_ascii_case("calc")
                        || ident.name.eq_ignore_ascii_case("-webkit-calc")
                        || ident.name.eq_ignore_ascii_case("-moz-calc")
                        || ident.name.eq_ignore_ascii_case("min")
                        || ident.name.eq_ignore_ascii_case("max")
                        || ident.name.eq_ignore_ascii_case("clamp")
                        || ident.name.eq_ignore_ascii_case("sin")
                        || ident.name.eq_ignore_ascii_case("cos")
                        || ident.name.eq_ignore_ascii_case("tan")
                        || ident.name.eq_ignore_ascii_case("asin")
                        || ident.name.eq_ignore_ascii_case("acos")
                        || ident.name.eq_ignore_ascii_case("atan")
                        || ident.name.eq_ignore_ascii_case("sqrt")
                        || ident.name.eq_ignore_ascii_case("exp")
                        || ident.name.eq_ignore_ascii_case("abs")
                        || ident.name.eq_ignore_ascii_case("sign")
                        || ident.name.eq_ignore_ascii_case("hypot")
                        || ident.name.eq_ignore_ascii_case("round")
                        || ident.name.eq_ignore_ascii_case("mod")
                        || ident.name.eq_ignore_ascii_case("rem")
                        || ident.name.eq_ignore_ascii_case("atan2")
                        || ident.name.eq_ignore_ascii_case("pow")
                        || ident.name.eq_ignore_ascii_case("log") =>
                {
                    // Only the legacy SassScript `min`/`max` accept the Sass `%` modulo
                    // operator; true calculations (`calc`, `clamp`, `sin`, ...) reject it.
                    let allow_modulo = matches!(self.syntax, Syntax::Scss | Syntax::Sass)
                        && (ident.name.eq_ignore_ascii_case("min")
                            || ident.name.eq_ignore_ascii_case("max"));
                    let mut values = self.vec_with_capacity(1);
                    loop {
                        match peek!(self) {
                            TokenWithSpan { token: Token::RParen(..), .. } => break,
                            TokenWithSpan { token: Token::Comma(..), .. } => {
                                values.push(ComponentValue::Delimiter(self.parse()?));
                            }
                            TokenWithSpan { token: Token::DotDotDot(..), .. }
                                if matches!(self.syntax, Syntax::Scss | Syntax::Sass)
                                    && values.len() == 1 =>
                            {
                                let TokenWithSpan { span: Span { end, .. }, .. } = bump!(self);
                                let value = values.remove(0);
                                let span = Span { start: value.span().start, end };
                                values.push(ComponentValue::SassArbitraryArgument(
                                    SassArbitraryArgument { value: arena_box!(self, value), span },
                                ));
                                break;
                            }
                            _ => values.push(self.parse_calc_expr(allow_modulo)?),
                        }
                    }
                    values
                }
                InterpolableIdent::Literal(ident) if ident.name.eq_ignore_ascii_case("element") => {
                    arena_vec!(self; self.parse().map(ComponentValue::IdSelector)?)
                }
                InterpolableIdent::Literal(Ident { raw: "boolean" | "if", .. })
                    if self.syntax == Syntax::Less =>
                {
                    let condition = ComponentValue::LessCondition(arena_box!(
                        self,
                        self.parse_less_condition(false)?
                    ));
                    let mut args = self.parse_function_args()?;
                    args.insert(0, condition);
                    args
                }
                _ => self.parse_function_args()?,
            }
        };
        let end = expect!(self, RParen).1.end;
        let span = Span { start: name.span().start, end };
        Ok(Function { name: FunctionName::Ident(name), args, span })
    }

    /// Parse a function with the typed grammar; when its arguments don't fit
    /// (dart-sass special functions carry raw text: `element(/**/ c)`,
    /// `-c-calc(@#$)`, `url(fn("s"))`), re-parse the contents as raw tokens.
    pub(super) fn parse_function_typed_or_raw(&mut self, name: Ident<'a>) -> PResult<Function<'a>> {
        let name_copy = Ident { name: name.name, raw: name.raw, span: name.span.clone() };
        match self.try_parse(|p| p.parse_function(InterpolableIdent::Literal(name))) {
            Ok(function) => Ok(function),
            Err(_) => self.parse_raw_function(name_copy),
        }
    }

    /// Parse `name(<raw contents>)` where the contents are preserved tokens
    /// balanced to the matching `)` (IE `expression(...)` and friends).
    fn parse_raw_function(&mut self, name: Ident<'a>) -> PResult<Function<'a>> {
        expect!(self, LParen);
        let mut args = self.vec_with_capacity(4);
        self.parse_raw_function_args_into(&mut args)?;
        let end = expect!(self, RParen).1.end;
        let span = Span { start: name.span.start, end };
        Ok(Function { name: FunctionName::Ident(InterpolableIdent::Literal(name)), args, span })
    }

    /// IE filter syntax `progid:DXImageTransform.Microsoft.f(...)`, optionally
    /// vendor prefixed: the `:dotted.path` prefix and the parenthesized
    /// contents are all preserved tokens.
    fn parse_progid_function(&mut self, name: Ident<'a>) -> PResult<Function<'a>> {
        let mut args = self.vec_with_capacity(4);
        loop {
            match &peek!(self).token {
                Token::LParen(..)
                | Token::Semicolon(..)
                | Token::RBrace(..)
                | Token::RParen(..)
                | Token::Eof(..)
                | Token::Indent(..)
                | Token::Dedent(..)
                | Token::Linebreak(..) => break,
                _ => args.push(ComponentValue::TokenWithSpan(bump!(self))),
            }
        }
        expect!(self, LParen);
        self.parse_raw_function_args_into(&mut args)?;
        let end = expect!(self, RParen).1.end;
        let span = Span { start: name.span.start, end };
        Ok(Function { name: FunctionName::Ident(InterpolableIdent::Literal(name)), args, span })
    }

    /// Consume function contents as preserved tokens, balancing pairs, until
    /// the function's own `)`. Semicolons and stray delimiters are legal here
    /// (`expression(a;b)`, `url(data:...;base64,...)`).
    fn parse_raw_function_args_into(
        &mut self,
        values: &mut oxc_allocator::Vec<'a, ComponentValue<'a>>,
    ) -> PResult<()> {
        let mut pairs: Vec<util::PairedToken> = Vec::with_capacity(1);
        loop {
            match &peek!(self).token {
                Token::Eof(..) => break,
                Token::LParen(..) => pairs.push(util::PairedToken::Paren),
                Token::RParen(..) => {
                    if let Some(util::PairedToken::Paren) = pairs.pop() {
                    } else {
                        break;
                    }
                }
                Token::LBracket(..) => pairs.push(util::PairedToken::Bracket),
                Token::RBracket(..) => {
                    if let Some(util::PairedToken::Bracket) = pairs.pop() {
                    } else {
                        break;
                    }
                }
                Token::LBrace(..) | Token::HashLBrace(..) => pairs.push(util::PairedToken::Brace),
                Token::RBrace(..) => {
                    if let Some(util::PairedToken::Brace) = pairs.pop() {
                    } else {
                        break;
                    }
                }
                // Interpolated strings must be parsed structurally so the
                // tokenizer resumes the string after each `#{...}`.
                Token::StrTemplate(..) => {
                    values.push(ComponentValue::InterpolableStr(self.parse()?));
                    continue;
                }
                _ => {}
            }
            values.push(ComponentValue::TokenWithSpan(bump!(self)));
        }
        Ok(())
    }

    pub(super) fn parse_function_args(
        &mut self,
    ) -> PResult<oxc_allocator::Vec<'a, ComponentValue<'a>>> {
        let mut values = self.vec_with_capacity(4);
        loop {
            match &peek!(self).token {
                Token::RParen(..) | Token::Eof(..) => break,
                Token::Semicolon(..) => {
                    values.push(self.parse().map(ComponentValue::Delimiter)?);
                }
                Token::Exclamation(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                    // while this syntax is weird, Bootstrap is actually using it
                    values.push(self.parse().map(ComponentValue::ImportantAnnotation)?);
                }
                Token::LBrace(..) if self.syntax == Syntax::Less => {
                    values.push(self.parse().map(ComponentValue::LessDetachedRuleset)?);
                }
                Token::Indent(..) | Token::Dedent(..) | Token::Linebreak(..) => {
                    bump!(self);
                }
                // A stray delimiter is a plain token in CSS, but the
                // preprocessor dialects give it real syntax and their
                // reference compilers reject it in function arguments.
                Token::Unknown(..) if self.syntax != Syntax::Css => {
                    let span = peek!(self).span.clone();
                    return Err(Error { kind: ErrorKind::UnknownToken, span });
                }
                _ => {
                    let value = if let Ok(value) = self.try_parse(ComponentValue::parse) {
                        value
                    } else {
                        values.push(ComponentValue::TokenWithSpan(bump!(self)));
                        continue;
                    };
                    if matches!(self.syntax, Syntax::Scss | Syntax::Sass) {
                        if let Some((_, mut span)) = eat!(self, DotDotDot) {
                            span.start = value.span().start;
                            values.push(ComponentValue::SassArbitraryArgument(
                                SassArbitraryArgument { value: arena_box!(self, value), span },
                            ));
                        } else if let ComponentValue::SassVariable(sass_var) = value {
                            if let Some((_, colon_span)) = eat!(self, Colon) {
                                let value = self.parse::<ComponentValue>()?;
                                let span =
                                    Span { start: sass_var.span.start, end: value.span().end };
                                values.push(ComponentValue::SassKeywordArgument(
                                    SassKeywordArgument {
                                        name: sass_var,
                                        colon_span,
                                        value: arena_box!(self, value),
                                        span,
                                    },
                                ));
                            } else {
                                values.push(ComponentValue::SassVariable(sass_var));
                            }
                        } else {
                            values.push(value);
                        }
                    } else {
                        values.push(value);
                    }
                }
            }
        }
        Ok(values)
    }

    pub(super) fn parse_ratio(&mut self, numerator: Number<'a>) -> PResult<Ratio<'a>> {
        let (_, solidus_span) = expect!(self, Solidus);
        let denominator = self.parse::<Number>()?;
        if denominator.value <= 0.0 {
            self.recoverable_errors.push(Error {
                kind: ErrorKind::InvalidRatioDenominator,
                span: denominator.span.clone(),
            });
        }

        let span = Span { start: numerator.span.start, end: denominator.span.end };
        Ok(Ratio { numerator, solidus_span, denominator, span })
    }

    fn parse_src_url(&mut self, name: Ident<'a>) -> PResult<Url<'a>> {
        // caller of `parse_src_url` should make sure there're no whitespaces before paren
        expect!(self, LParen);
        let value = match &peek!(self).token {
            Token::Str(..) | Token::StrTemplate(..) => {
                Some(UrlValue::Str(self.parse::<InterpolableStr>()?))
            }
            _ => None,
        };
        let modifiers = match &peek!(self).token {
            Token::Ident(..) | Token::HashLBrace(..) | Token::AtLBraceVar(..) => {
                let mut modifiers = self.vec_with_capacity(1);
                loop {
                    modifiers.push(self.parse()?);
                    if let Token::RParen(..) = &peek!(self).token {
                        break;
                    }
                }
                modifiers
            }
            _ => arena_vec!(self),
        };
        let end = expect!(self, RParen).1.end;
        let span = Span { start: name.span.start, end };
        Ok(Url { name, value, modifiers, span })
    }

    fn parse_unicode_range(&mut self, prefix_ident: Ident<'a>) -> PResult<UnicodeRange<'a>> {
        let prefix = prefix_ident.raw.chars().next().unwrap();
        let (span_start, span_end) = match bump!(self) {
            TokenWithSpan { token: Token::Plus(..), span: plus_token_span } => {
                let start = plus_token_span.start;
                let mut end = match self.tokenizer.bump_without_ws_or_comments()? {
                    TokenWithSpan { token: Token::Ident(..) | Token::Question(..), span } => {
                        span.end
                    }
                    TokenWithSpan { token, span } => {
                        return Err(Error {
                            kind: ErrorKind::Unexpected("?", token.symbol()),
                            span,
                        });
                    }
                };
                loop {
                    match peek!(self) {
                        TokenWithSpan { token: Token::Question(..), span } if span.start == end => {
                            end = bump!(self).span.end;
                        }
                        _ => break,
                    }
                }
                (start, end)
            }
            TokenWithSpan { token: Token::Dimension(..), span: dimension_token_span } => {
                let start = dimension_token_span.start;
                let mut end = dimension_token_span.end;
                loop {
                    match peek!(self) {
                        TokenWithSpan { token: Token::Question(..), span } if span.start == end => {
                            end = bump!(self).span.end;
                        }
                        _ => break,
                    }
                }
                (start, end)
            }
            TokenWithSpan { token: Token::Number(..), span: number_token_span } => {
                let start = number_token_span.start;
                let mut end = number_token_span.end;
                match &peek!(self).token {
                    Token::Question(..) => {
                        end = bump!(self).span.end;
                        loop {
                            match peek!(self) {
                                TokenWithSpan { token: Token::Question(..), span }
                                    if span.start == end =>
                                {
                                    end = bump!(self).span.end;
                                }
                                _ => break,
                            }
                        }
                    }
                    Token::Dimension(..) | Token::Number(..) => {
                        end = bump!(self).span.end;
                    }
                    _ => {}
                }
                (start, end)
            }
            TokenWithSpan { span, .. } => {
                return Err(Error { kind: ErrorKind::InvalidUnicodeRange, span });
            }
        };

        let source = self.source.get(span_start + 1..span_end).ok_or(Error {
            kind: ErrorKind::InvalidUnicodeRange,
            span: Span { start: span_start + 1, end: span_end },
        })?;
        let span = Span { start: prefix_ident.span.start, end: span_end };
        let unicode_range = if let Some((left, right)) = source.split_once('-') {
            if left.len() > 6 || !left.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(Error { kind: ErrorKind::InvalidUnicodeRange, span });
            }
            if right.len() > 6
                || !right.trim_end_matches('?').chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(Error { kind: ErrorKind::InvalidUnicodeRange, span });
            }
            let start = u32::from_str_radix(left, 16)
                .map_err(|_| Error { kind: ErrorKind::InvalidUnicodeRange, span: span.clone() })?;
            let end = u32::from_str_radix(&replace_unicode_range_wildcards(right, 'F'), 16)
                .map_err(|_| Error { kind: ErrorKind::InvalidUnicodeRange, span: span.clone() })?;
            UnicodeRange { prefix, start, start_raw: left, end, end_raw: Some(right), span }
        } else {
            if source.len() > 6
                || !source.trim_end_matches('?').chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(Error { kind: ErrorKind::InvalidUnicodeRange, span });
            }
            let start = u32::from_str_radix(&replace_unicode_range_wildcards(source, '0'), 16)
                .map_err(|_| Error { kind: ErrorKind::InvalidUnicodeRange, span: span.clone() })?;
            let end = u32::from_str_radix(&replace_unicode_range_wildcards(source, 'F'), 16)
                .map_err(|_| Error { kind: ErrorKind::InvalidUnicodeRange, span: span.clone() })?;
            UnicodeRange { prefix, start, start_raw: source, end, end_raw: None, span }
        };
        if unicode_range.end > 0x10ffff {
            self.recoverable_errors.push(Error {
                kind: ErrorKind::MaxCodePointExceeded,
                span: unicode_range.span.clone(),
            });
        }
        if unicode_range.start > unicode_range.end {
            self.recoverable_errors.push(Error {
                kind: ErrorKind::UnicodeRangeStartGreaterThanEnd,
                span: unicode_range.span.clone(),
            });
        }
        Ok(unicode_range)
    }
}

fn replace_unicode_range_wildcards(source: &str, replacement: char) -> String {
    source.chars().map(|c| if c == '?' { replacement } else { c }).collect()
}

impl<'a> Parse<'a> for BracketBlock<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let start = expect!(input, LBracket).1.start;
        let mut value = input.vec_with_capacity(3);
        loop {
            match &peek!(input).token {
                Token::RBracket(..) => break,
                _ => value.push(input.parse()?),
            }
        }
        let end = expect!(input, RBracket).1.end;
        Ok(BracketBlock { value, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for ComponentValue<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match input.syntax {
            Syntax::Css => input.parse_component_value_atom(),
            Syntax::Scss | Syntax::Sass => {
                input.parse_sass_bin_expr(/* allow_comparison */ true)
            }
            Syntax::Less => input.parse_less_operation(/* allow_mixin_call */ true),
        }
    }
}

impl<'a> Parse<'a> for ComponentValues<'a> {
    /// This is for public-use only. For internal code of oxc-css-parser, **DO NOT** use.
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let first = input.parse::<ComponentValue>()?;
        let mut span = first.span().clone();

        let mut values = input.vec_with_capacity(4);
        values.push(first);
        loop {
            match &peek!(input).token {
                Token::Eof(..) => break,
                Token::Semicolon(..) => {
                    values.push(input.parse().map(ComponentValue::Delimiter)?);
                }
                _ => values.push(input.parse()?),
            }
        }

        if let Some(value) = values.last() {
            span.end = value.span().end;
        }
        Ok(ComponentValues { values, span })
    }
}

impl<'a> Parse<'a> for Delimiter {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        use crate::tokenizer::token::*;
        match bump!(input) {
            TokenWithSpan { token: Token::Solidus(..), span } => {
                Ok(Delimiter { kind: DelimiterKind::Solidus, span })
            }
            TokenWithSpan { token: Token::Comma(..), span } => {
                Ok(Delimiter { kind: DelimiterKind::Comma, span })
            }
            TokenWithSpan { token: Token::Semicolon(..), span } => {
                Ok(Delimiter { kind: DelimiterKind::Semicolon, span })
            }
            _ => unreachable!(),
        }
    }
}

impl<'a> Parse<'a> for Dimension<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (dimension, span) = expect!(input, Dimension);
        input.dimension(dimension, span)
    }
}

impl<'a> Parse<'a> for Function<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let name = input.parse::<FunctionName>()?;
        match peek!(input) {
            TokenWithSpan { token: Token::LParen(..), span } => {
                util::assert_no_ws_or_comment(name.span(), span)?;
                match name {
                    FunctionName::Ident(name) => input.parse_function(name),
                    name => {
                        bump!(input);
                        let args = input.parse_function_args()?;
                        let (_, Span { end, .. }) = expect!(input, RParen);
                        let span = Span { start: name.span().start, end };
                        Ok(Function { name, args, span })
                    }
                }
            }
            TokenWithSpan { token, span } => {
                use crate::{token::LParen, tokenizer::TokenSymbol};
                Err(Error {
                    kind: ErrorKind::Unexpected(LParen::symbol(), token.symbol()),
                    span: span.clone(),
                })
            }
        }
    }
}

impl<'a> Parse<'a> for FunctionName<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match peek!(input).token {
            Token::Ident(..) => {
                let ident = input.parse::<Ident>()?;
                match (&peek!(input).token, input.syntax) {
                    (Token::Dot(..), Syntax::Scss | Syntax::Sass) => {
                        bump!(input);
                        let member = input.parse::<Ident>()?;
                        let span = Span { start: ident.span.start, end: member.span.end };
                        Ok(FunctionName::SassQualifiedName(arena_box!(
                            input,
                            SassQualifiedName {
                                module: ident,
                                member: SassModuleMemberName::Ident(member),
                                span,
                            }
                        )))
                    }
                    _ => Ok(FunctionName::Ident(InterpolableIdent::Literal(ident))),
                }
            }
            Token::Percent(..) if input.syntax == Syntax::Less => {
                input.parse().map(FunctionName::LessFormatFunction)
            }
            Token::Tilde(..) if input.syntax == Syntax::Less => {
                input.parse().map(FunctionName::LessListFunction)
            }
            _ => {
                use crate::{token::Ident, tokenizer::TokenSymbol};
                let TokenWithSpan { token, span } = bump!(input);
                Err(Error { kind: ErrorKind::Unexpected(Ident::symbol(), token.symbol()), span })
            }
        }
    }
}

impl<'a> Parse<'a> for HexColor<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (token, span) = expect!(input, Hash);
        let raw = token.raw;
        let value =
            if token.escaped { util::handle_escape_in(raw, input.allocator()) } else { raw };
        Ok(HexColor { value, raw, span })
    }
}

impl<'a> Parse<'a> for Ident<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (ident, span) = expect!(input, Ident);
        Ok(input.ident(ident, span))
    }
}

impl<'a> Parse<'a> for InterpolableIdent<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        // A css-in-js placeholder stands in for an interpolated ident anywhere one
        // is expected (id selector `#${x}`, attribute value `[a=${x}]`, ...).
        if let Token::Placeholder(..) = peek!(input).token {
            let (placeholder, span) = expect!(input, Placeholder);
            return Ok(InterpolableIdent::Placeholder((placeholder, span).into()));
        }
        match input.syntax {
            Syntax::Css => input.parse().map(InterpolableIdent::Literal),
            Syntax::Scss | Syntax::Sass => input.parse_sass_interpolated_ident(),
            Syntax::Less => {
                // Less variable interpolation is disallowed in declaration value
                if matches!(
                    input.state.qualified_rule_ctx,
                    Some(QualifiedRuleContext::DeclarationValue)
                ) {
                    input.parse().map(InterpolableIdent::Literal)
                } else {
                    input.parse_less_interpolated_ident()
                }
            }
        }
    }
}

impl<'a> Parse<'a> for InterpolableStr<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match peek!(input) {
            TokenWithSpan { token: Token::Str(..), .. } => {
                input.parse().map(InterpolableStr::Literal)
            }
            TokenWithSpan { token: Token::StrTemplate(..), span } => match input.syntax {
                Syntax::Scss | Syntax::Sass => input.parse().map(InterpolableStr::SassInterpolated),
                Syntax::Less => input.parse().map(InterpolableStr::LessInterpolated),
                Syntax::Css => {
                    Err(Error { kind: ErrorKind::UnexpectedTemplateInCss, span: span.clone() })
                }
            },
            TokenWithSpan { span, .. } => {
                Err(Error { kind: ErrorKind::ExpectString, span: span.clone() })
            }
        }
    }
}

impl<'a> Parse<'a> for Number<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (number, span) = expect!(input, Number);
        number
            .raw
            .parse()
            .map_err(|_| Error { kind: ErrorKind::InvalidNumber, span: span.clone() })
            .map(|value| Self { value, raw: number.raw, span })
    }
}

impl<'a> Parse<'a> for Percentage<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (token, span) = expect!(input, Percentage);
        Ok(Percentage {
            value: (token.value, Span { start: span.start, end: span.end - 1 }).try_into()?,
            span,
        })
    }
}

impl<'a> Parse<'a> for Str<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (str, span) = expect!(input, Str);
        Ok(input.str(str, span))
    }
}

impl<'a> Parse<'a> for Url<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (prefix, prefix_span) = expect!(input, Ident);
        if !prefix.name().eq_ignore_ascii_case("url") {
            return Err(Error { kind: ErrorKind::ExpectUrl, span: prefix_span });
        }
        let prefix_start = prefix_span.start;
        let name = input.ident(prefix, prefix_span.clone());

        match peek!(input) {
            TokenWithSpan { token: Token::LParen(..), span } if prefix_span.end == span.start => {
                bump!(input);
            }
            TokenWithSpan { span, .. } => {
                return Err(Error { kind: ErrorKind::TryParseError, span: span.clone() });
            }
        }

        if input.tokenizer.is_start_of_url_string() {
            let value = input.parse()?;
            let modifiers = match &peek!(input).token {
                Token::Ident(..) | Token::HashLBrace(..) | Token::AtLBraceVar(..) => {
                    let mut modifiers = input.vec_with_capacity(1);
                    loop {
                        modifiers.push(input.parse()?);
                        if let Token::RParen(..) = &peek!(input).token {
                            break;
                        }
                    }
                    modifiers
                }
                _ => arena_vec!(input),
            };
            let end = expect!(input, RParen).1.end;
            let span = Span { start: prefix_start, end };
            Ok(Url { name, value: Some(UrlValue::Str(value)), modifiers, span })
        } else if let Ok(value) = input.try_parse(UrlRaw::parse) {
            let span = Span {
                start: prefix_start,
                end: value.span.end + 1, // `)` is consumed, but span excludes it
            };
            Ok(Url { name, value: Some(UrlValue::Raw(value)), modifiers: arena_vec!(input), span })
        } else {
            match input.syntax {
                Syntax::Css => Err(Error { kind: ErrorKind::InvalidUrl, span: bump!(input).span }),
                Syntax::Scss | Syntax::Sass => {
                    let value = input.parse::<SassInterpolatedUrl>()?;
                    let span = Span {
                        start: prefix_start,
                        end: value.span.end + 1, // `)` is consumed, but span excludes it
                    };
                    Ok(Url {
                        name,
                        value: Some(UrlValue::SassInterpolated(value)),
                        modifiers: arena_vec!(input),
                        span,
                    })
                }
                Syntax::Less => {
                    let value = UrlValue::LessEscapedStr(input.parse()?);
                    let (_, Span { end, .. }) = expect!(input, RParen);
                    let span = Span { start: prefix_start, end };
                    Ok(Url { name, value: Some(value), modifiers: arena_vec!(input), span })
                }
            }
        }
    }
}

impl<'a> Parse<'a> for UrlModifier<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let ident = input.parse::<InterpolableIdent>()?;
        match peek!(input) {
            TokenWithSpan { token: Token::LParen(..), span } if ident.span().end == span.start => {
                input.parse_function(ident).map(UrlModifier::Function)
            }
            _ => Ok(UrlModifier::Ident(ident)),
        }
    }
}

impl<'a> Parse<'a> for UrlRaw<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match input.tokenizer.scan_url_raw_or_template()? {
            TokenWithSpan { token: Token::UrlRaw(url), span } => {
                let value = if url.escaped {
                    util::handle_escape_in(url.raw, input.allocator())
                } else {
                    url.raw
                };
                Ok(UrlRaw { value, raw: url.raw, span })
            }
            TokenWithSpan { token, span } => {
                Err(Error { kind: ErrorKind::Unexpected("<url>", token.symbol()), span })
            }
        }
    }
}

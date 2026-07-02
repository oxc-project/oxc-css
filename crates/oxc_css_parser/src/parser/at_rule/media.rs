use super::Parser;
use crate::{
    Parse, Syntax, arena_box, arena_vec,
    ast::*,
    bump, eat,
    error::{Error, ErrorKind, PResult},
    expect, peek,
    pos::{Span, Spanned},
    tokenizer::{Token, TokenWithSpan},
};

impl<'a> Parse<'a> for MediaAnd<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let keyword = input.parse::<Ident>()?;
        if keyword.name.eq_ignore_ascii_case("and") {
            let media_in_parens = input.parse_media_in_parens_after_logic()?;
            let span = Span { start: keyword.span.start, end: media_in_parens.span.end };
            Ok(MediaAnd { keyword, media_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectMediaAnd, span: keyword.span })
        }
    }
}

impl<'a> Parse<'a> for MediaConditionAfterMediaType<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let and: Ident = match bump!(input) {
            TokenWithSpan { token: Token::Ident(ident), span }
                if ident.name().eq_ignore_ascii_case("and") =>
            {
                input.ident(ident, span)
            }
            TokenWithSpan { span, .. } => {
                return Err(Error { kind: ErrorKind::ExpectMediaAnd, span });
            }
        };

        let condition = input.parse_media_condition(
            /* allow_or */ false, /* after_logic_keyword */ true,
        )?;

        let span = Span { start: and.span.start, end: condition.span.end };
        Ok(MediaConditionAfterMediaType { and, condition, span })
    }
}

impl<'a> Parse<'a> for MediaFeature<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match input.parse_media_feature_value()? {
            ComponentValue::InterpolableIdent(ident) => match &peek!(input).token {
                Token::Colon(..) => input.parse_media_feature_plain(ident).map(MediaFeature::Plain),
                Token::LessThan(..)
                | Token::LessThanEqual(..)
                | Token::GreaterThan(..)
                | Token::GreaterThanEqual(..)
                | Token::Equal(..) => input.parse_media_feature_range_or_range_interval(
                    ComponentValue::InterpolableIdent(ident),
                ),
                _ => {
                    let span = ident.span().clone();
                    Ok(MediaFeature::Boolean(MediaFeatureBoolean {
                        name: MediaFeatureName::Ident(ident),
                        span,
                    }))
                }
            },
            ComponentValue::SassVariable(variable) => {
                let span = variable.span.clone();
                Ok(MediaFeature::Boolean(MediaFeatureBoolean {
                    name: MediaFeatureName::SassVariable(variable),
                    span,
                }))
            }
            ComponentValue::PostcssSimpleVar(variable) => {
                let span = variable.span.clone();
                Ok(MediaFeature::Boolean(MediaFeatureBoolean {
                    name: MediaFeatureName::PostcssSimpleVar(variable),
                    span,
                }))
            }
            value => input.parse_media_feature_range_or_range_interval(value),
        }
    }
}

impl<'a> Parse<'a> for MediaFeatureComparison {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match bump!(input) {
            TokenWithSpan { token: Token::LessThan(..), span } => {
                Ok(MediaFeatureComparison { kind: MediaFeatureComparisonKind::LessThan, span })
            }
            TokenWithSpan { token: Token::LessThanEqual(..), span } => Ok(MediaFeatureComparison {
                kind: MediaFeatureComparisonKind::LessThanOrEqual,
                span,
            }),
            TokenWithSpan { token: Token::GreaterThan(..), span } => {
                Ok(MediaFeatureComparison { kind: MediaFeatureComparisonKind::GreaterThan, span })
            }
            TokenWithSpan { token: Token::GreaterThanEqual(..), span } => {
                Ok(MediaFeatureComparison {
                    kind: MediaFeatureComparisonKind::GreaterThanOrEqual,
                    span,
                })
            }
            TokenWithSpan { token: Token::Equal(..), span } => {
                Ok(MediaFeatureComparison { kind: MediaFeatureComparisonKind::Equal, span })
            }
            TokenWithSpan { span, .. } => {
                Err(Error { kind: ErrorKind::ExpectMediaFeatureComparison, span })
            }
        }
    }
}

impl<'a> Parse<'a> for MediaInParens<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        // Sass allows an interpolation wherever `<media-in-parens>` is expected,
        // e.g. `@media screen and #{$query} {}`.
        if matches!(input.syntax, Syntax::Scss | Syntax::Sass)
            && matches!(&peek!(input).token, Token::HashLBrace(..))
            && let InterpolableIdent::SassInterpolated(interpolation) =
                input.parse_sass_interpolated_ident()?
        {
            let span = interpolation.span.clone();
            return Ok(MediaInParens {
                kind: MediaInParensKind::SassInterpolation(interpolation),
                span,
            });
        }
        let (_, Span { start, .. }) = expect!(input, LParen);
        let kind = input.parse()?;
        let (_, Span { end, .. }) = expect!(input, RParen);
        Ok(MediaInParens { kind, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for MediaInParensKind<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        if let Ok(media_condition) = input.try_parse(|parser| {
            let media_condition = parser.parse_media_condition(
                /* allow_or */ true, /* after_logic_keyword */ false,
            )?;
            // `(#{$x}-width: 1px)`: the interpolation parses as a
            // `SassInterpolation` media condition, but a trailing `:` means it is
            // really a media feature name. Require the closing `)` here so such
            // cases fall through to the media-feature branch below.
            if matches!(&peek!(parser).token, Token::RParen(..)) {
                Ok(media_condition)
            } else {
                let span = peek!(parser).span.clone();
                Err(Error { kind: ErrorKind::ExpectMediaFeatureName, span })
            }
        }) {
            Ok(MediaInParensKind::MediaCondition(media_condition))
        } else if let Ok(media_feature) = input.try_parse(|parser| {
            let media_feature = parser.parse::<MediaFeature>()?;
            if matches!(&peek!(parser).token, Token::RParen(..)) {
                Ok(media_feature)
            } else {
                let span = peek!(parser).span.clone();
                Err(Error { kind: ErrorKind::ExpectMediaFeatureName, span })
            }
        }) {
            Ok(MediaInParensKind::MediaFeature(arena_box!(input, media_feature)))
        } else {
            // <general-enclosed>: MQ L4 forward-compat catch-all, evaluates false at runtime.
            let tokens = input.parse_tokens_in_parens()?;
            Ok(MediaInParensKind::GeneralEnclosed(tokens))
        }
    }
}

impl<'a> Parse<'a> for MediaNot<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let keyword = input.parse::<Ident>()?;
        if keyword.name.eq_ignore_ascii_case("not") {
            let media_in_parens = input.parse::<MediaInParens>()?;
            let span = Span { start: keyword.span.start, end: media_in_parens.span.end };
            Ok(MediaNot { keyword, media_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectMediaNot, span: keyword.span })
        }
    }
}

impl<'a> Parse<'a> for MediaOr<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let keyword = input.parse::<Ident>()?;
        if keyword.name.eq_ignore_ascii_case("or") {
            let media_in_parens = input.parse_media_in_parens_after_logic()?;
            let span = Span { start: keyword.span.start, end: media_in_parens.span.end };
            Ok(MediaOr { keyword, media_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectMediaOr, span: keyword.span })
        }
    }
}

impl<'a> Parse<'a> for MediaQuery<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        if let Ok(condition_only) = input.try_parse(|parser| {
            parser.parse_media_condition(
                /* allow_or */ true, /* after_logic_keyword */ false,
            )
        }) {
            Ok(MediaQuery::ConditionOnly(condition_only))
        } else if input.syntax == Syntax::Less {
            match peek!(input).token {
                Token::AtKeyword(..) => {
                    input.parse_less_maybe_variable_or_with_lookups().map(|value| match value {
                        ComponentValue::LessVariable(variable) => {
                            MediaQuery::LessVariable(variable)
                        }
                        ComponentValue::LessNamespaceValue(namespace_value) => {
                            MediaQuery::LessNamespaceValue(namespace_value)
                        }
                        _ => unreachable!(),
                    })
                }
                Token::Dot(..) | Token::Hash(..) => {
                    let less_namespace_value = input.parse()?;
                    Ok(MediaQuery::LessNamespaceValue(arena_box!(input, less_namespace_value)))
                }
                _ => input.parse_media_query_with_type_or_function(),
            }
        } else {
            input.parse_media_query_with_type_or_function()
        }
    }
}

// https://www.w3.org/TR/mediaqueries-4/#mq-syntax
impl<'a> Parse<'a> for MediaQueryList<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let first = input.parse_media_query_or_unknown(/* has_siblings */ false)?;
        let mut span = first.span().clone();

        let mut queries = arena_vec!(input; first);
        let mut comma_spans = arena_vec!(input);
        while let Some((_, comma_span)) = eat!(input, Comma) {
            comma_spans.push(comma_span);
            queries.push(input.parse_media_query_or_unknown(/* has_siblings */ true)?);
        }
        debug_assert_eq!(comma_spans.len() + 1, queries.len());

        // SAFETY: it has at least one element.
        span.end = unsafe {
            let index = queries.len() - 1;
            queries.get_unchecked(index).span().end
        };
        Ok(MediaQueryList { queries, comma_spans, span })
    }
}

impl<'a> Parse<'a> for MediaQueryWithType<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let modifier = if let Token::Ident(ident) = &peek!(input).token {
            let name = ident.name();
            if name.eq_ignore_ascii_case("not") || name.eq_ignore_ascii_case("only") {
                Some(input.parse::<Ident>()?)
            } else {
                None
            }
        } else {
            None
        };
        let media_type = input.parse::<InterpolableIdent>()?;
        if let InterpolableIdent::Literal(Ident { name, span, .. }) = &media_type
            && (name.eq_ignore_ascii_case("only")
                || name.eq_ignore_ascii_case("not")
                || name.eq_ignore_ascii_case("and")
                || name.eq_ignore_ascii_case("or")
                || name.eq_ignore_ascii_case("layer"))
        {
            input.recoverable_errors.push(Error {
                kind: ErrorKind::MediaTypeKeywordDisallowed(name.to_string()),
                span: span.clone(),
            });
        }
        let condition = match &peek!(input).token {
            Token::Ident(ident) if ident.name().eq_ignore_ascii_case("and") => {
                input.parse::<MediaConditionAfterMediaType>().map(Some)?
            }
            _ => None,
        };

        let mut span = media_type.span().clone();
        if let Some(modifier) = &modifier {
            span.start = modifier.span.start;
        }
        if let Some(condition) = &condition {
            span.end = condition.span.end;
        }
        Ok(MediaQueryWithType { modifier, media_type, condition, span })
    }
}

impl<'a> Parser<'a> {
    /// One query of a media query list. In CSS an unparseable query inside a
    /// *list* does not discard the rule — it merely evaluates to "not all"
    /// (`@media &test, speech {}` still applies to speech) — so on failure
    /// the query's raw tokens are preserved as [`MediaQuery::Unknown`]. A
    /// sole invalid query stays an error (dart-sass rejects it in plain CSS
    /// too), which is why list membership is required: the query must follow
    /// or be followed by a comma.
    fn parse_media_query_or_unknown(&mut self, has_siblings: bool) -> PResult<MediaQuery<'a>> {
        let error = match self.try_parse(MediaQuery::parse) {
            Ok(query) => return Ok(query),
            Err(error) => error,
        };
        if self.syntax != Syntax::Css {
            return Err(error);
        }
        let start = peek!(self).span.start;
        let mut tokens = arena_vec!(self);
        let mut pairs = Vec::new();
        loop {
            match &peek!(self).token {
                Token::Eof(..) | Token::Semicolon(..) => break,
                Token::Comma(..) | Token::LBrace(..) if pairs.is_empty() => break,
                token => {
                    if !crate::util::track_paired_token(token, &mut pairs) {
                        break;
                    }
                }
            }
            tokens.push(bump!(self));
        }
        if !has_siblings && !matches!(peek!(self).token, Token::Comma(..)) {
            return Err(error);
        }
        let Some(end) = tokens.last().map(|t| t.span.end) else {
            return Err(error);
        };
        Ok(MediaQuery::Unknown(TokenSeq { tokens, span: Span { start, end } }))
    }

    /// `<media-in-parens>` right after `and`/`or`. A bare ident there
    /// (`@media screen and print`) is not valid MQ syntax, but browsers still
    /// parse the rule — the query merely evaluates to "not all" — so preserve
    /// it like `<general-enclosed>` raw tokens. Only this position is safe to
    /// relax: elsewhere an ident is a media type (`@media print`).
    fn parse_media_in_parens_after_logic(&mut self) -> PResult<MediaInParens<'a>> {
        if self.syntax == Syntax::Css
            && let TokenWithSpan { token: Token::Ident(ident), span } = peek!(self)
            && !ident.name().eq_ignore_ascii_case("not")
            && self.source.as_bytes().get(span.end) != Some(&b'(')
        {
            let token = bump!(self);
            let span = token.span.clone();
            return Ok(MediaInParens {
                kind: MediaInParensKind::GeneralEnclosed(TokenSeq {
                    tokens: arena_vec!(self; token),
                    span: span.clone(),
                }),
                span,
            });
        }
        self.parse()
    }

    fn parse_media_condition(
        &mut self,
        allow_or: bool,
        after_logic_keyword: bool,
    ) -> PResult<MediaCondition<'a>> {
        match &peek!(self).token {
            Token::Ident(ident) if ident.name().eq_ignore_ascii_case("not") => {
                let media_not = self.parse::<MediaNot>()?;
                let span = media_not.span.clone();
                Ok(MediaCondition {
                    conditions: arena_vec!(self; MediaConditionKind::Not(media_not)),
                    span,
                })
            }
            _ => {
                let first = if after_logic_keyword {
                    self.parse_media_in_parens_after_logic()?
                } else {
                    self.parse::<MediaInParens>()?
                };
                let mut span = first.span.clone();
                let mut conditions = arena_vec!(self; MediaConditionKind::MediaInParens(first));
                if let Token::Ident(ident) = &peek!(self).token {
                    let name = ident.name();
                    if name.eq_ignore_ascii_case("and") {
                        loop {
                            conditions.push(MediaConditionKind::And(self.parse()?));
                            match &peek!(self).token {
                                Token::Ident(ident) if ident.name().eq_ignore_ascii_case("and") => {
                                }
                                _ => break,
                            }
                        }
                    } else if allow_or && name.eq_ignore_ascii_case("or") {
                        loop {
                            conditions.push(MediaConditionKind::Or(self.parse()?));
                            match &peek!(self).token {
                                Token::Ident(ident) if ident.name().eq_ignore_ascii_case("or") => {}
                                _ => break,
                            }
                        }
                    }
                }

                if let Some(last) = conditions.last() {
                    span.end = last.span().end;
                }
                Ok(MediaCondition { conditions, span })
            }
        }
    }

    fn parse_media_feature_plain(
        &mut self,
        ident: InterpolableIdent<'a>,
    ) -> PResult<MediaFeaturePlain<'a>> {
        let (_, colon_span) = expect!(self, Colon);
        let value = self.parse_media_feature_value()?;
        let span = Span { start: ident.span().start, end: value.span().end };
        Ok(MediaFeaturePlain { name: MediaFeatureName::Ident(ident), colon_span, value, span })
    }

    fn parse_media_feature_range_or_range_interval(
        &mut self,
        left: ComponentValue<'a>,
    ) -> PResult<MediaFeature<'a>> {
        let comparison = self.parse()?;
        let name_or_right = self.parse_media_feature_value()?;
        if let ComponentValue::InterpolableIdent(ident) = name_or_right {
            match &peek!(self).token {
                Token::LessThan(..)
                | Token::LessThanEqual(..)
                | Token::GreaterThan(..)
                | Token::GreaterThanEqual(..)
                | Token::Equal(..) => {
                    let right_comparison = self.parse()?;
                    let right = self.parse_media_feature_value()?;
                    let span = Span { start: left.span().start, end: right.span().end };
                    Ok(MediaFeature::RangeInterval(MediaFeatureRangeInterval {
                        left,
                        left_comparison: comparison,
                        name: MediaFeatureName::Ident(ident),
                        right_comparison,
                        right,
                        span,
                    }))
                }
                _ => {
                    let span = Span { start: left.span().start, end: ident.span().end };
                    Ok(MediaFeature::Range(MediaFeatureRange {
                        left,
                        comparison,
                        right: ComponentValue::InterpolableIdent(ident),
                        span,
                    }))
                }
            }
        } else {
            if !matches!(left, ComponentValue::InterpolableIdent(..))
                && !matches!(name_or_right, ComponentValue::InterpolableIdent(..))
            {
                self.recoverable_errors.push(Error {
                    kind: ErrorKind::ExpectMediaFeatureName,
                    span: name_or_right.span().clone(),
                });
            }
            let span = Span { start: left.span().start, end: name_or_right.span().end };
            Ok(MediaFeature::Range(MediaFeatureRange {
                left,
                comparison,
                right: name_or_right,
                span,
            }))
        }
    }

    fn parse_media_feature_value(&mut self) -> PResult<ComponentValue<'a>> {
        let value = match self.syntax {
            Syntax::Css => self.parse_component_value_atom()?,
            Syntax::Scss | Syntax::Sass => {
                self.parse_sass_bin_expr(/* allow_comparison */ false)?
            }
            Syntax::Less => self.parse_less_operation(/* allow_mixin_call */ true)?,
        };
        match value {
            ComponentValue::Number(number)
                if number.value >= 0.0 && matches!(peek!(self).token, Token::Solidus(..)) =>
            {
                self.parse_ratio(number).map(ComponentValue::Ratio)
            }
            value => Ok(value),
        }
    }

    fn parse_media_query_with_type_or_function(&mut self) -> PResult<MediaQuery<'a>> {
        let media_query_with_type = self.parse::<MediaQueryWithType>()?;
        match (media_query_with_type, peek!(self)) {
            (
                MediaQueryWithType {
                    modifier: None,
                    media_type: name,
                    condition: None,
                    span: mq_span,
                },
                TokenWithSpan { token: crate::token::Token::LParen(..), span: lparen_span },
            ) if mq_span.end == lparen_span.start => {
                bump!(self);
                let args = self.parse_function_args()?;
                let (_, Span { end, .. }) = expect!(self, RParen);
                Ok(MediaQuery::Function(Function {
                    name: FunctionName::Ident(name),
                    args,
                    span: Span { start: mq_span.start, end },
                }))
            }
            (media_query_with_type, _) => Ok(MediaQuery::WithType(media_query_with_type)),
        }
    }
}

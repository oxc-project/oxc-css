use super::Parser;
use crate::{
    Parse, arena_box, arena_vec,
    ast::*,
    eat,
    error::{Error, ErrorKind, PResult},
    expect, expect_without_ws_or_comments, peek,
    pos::{Span, Spanned},
    tokenizer::{Token, TokenWithSpan},
};

impl<'a> Parse<'a> for ContainerCondition<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match &peek!(input).token {
            Token::Ident(ident) if ident.name().eq_ignore_ascii_case("not") => {
                let container_condition_not = input.parse::<ContainerConditionNot>()?;
                let span = container_condition_not.span.clone();
                Ok(ContainerCondition {
                    conditions: arena_vec!(input; ContainerConditionKind::Not(container_condition_not)),
                    span,
                })
            }
            _ => {
                let first = input.parse::<QueryInParens>()?;
                let mut span = first.span.clone();
                let mut conditions =
                    arena_vec!(input; ContainerConditionKind::QueryInParens(first));
                // formally `and`/`or` may not mix without parens and `not`
                // is leading-only, but real-world code (less.js) chains them
                // freely: `(a) or (b) and (c)`, `(a) not (b)`.
                loop {
                    let kind = match &peek!(input).token {
                        Token::Ident(ident) if ident.name().eq_ignore_ascii_case("and") => {
                            ContainerConditionKind::And(input.parse()?)
                        }
                        Token::Ident(ident) if ident.name().eq_ignore_ascii_case("or") => {
                            ContainerConditionKind::Or(input.parse()?)
                        }
                        Token::Ident(ident) if ident.name().eq_ignore_ascii_case("not") => {
                            ContainerConditionKind::Not(input.parse()?)
                        }
                        _ => break,
                    };
                    conditions.push(kind);
                }

                if let Some(last) = conditions.last() {
                    span.end = last.span().end;
                }
                Ok(ContainerCondition { conditions, span })
            }
        }
    }
}

impl<'a> Parse<'a> for ContainerConditionAnd<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let keyword = input.parse::<Ident>()?;
        if keyword.name.eq_ignore_ascii_case("and") {
            let query_in_parens = input.parse::<QueryInParens>()?;
            let span = Span { start: keyword.span.start, end: query_in_parens.span.end };
            Ok(ContainerConditionAnd { keyword, query_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectContainerConditionAnd, span: keyword.span })
        }
    }
}

impl<'a> Parse<'a> for ContainerConditionNot<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let keyword = input.parse::<Ident>()?;
        if keyword.name.eq_ignore_ascii_case("not") {
            let query_in_parens = input.parse::<QueryInParens>()?;
            let span = Span { start: keyword.span.start, end: query_in_parens.span.end };
            Ok(ContainerConditionNot { keyword, query_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectContainerConditionNot, span: keyword.span })
        }
    }
}

impl<'a> Parse<'a> for ContainerConditionOr<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let keyword = input.parse::<Ident>()?;
        if keyword.name.eq_ignore_ascii_case("or") {
            let query_in_parens = input.parse::<QueryInParens>()?;
            let span = Span { start: keyword.span.start, end: query_in_parens.span.end };
            Ok(ContainerConditionOr { keyword, query_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectContainerConditionOr, span: keyword.span })
        }
    }
}

impl<'a> Parse<'a> for QueryInParens<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        if let Some((_, Span { start, .. })) = eat!(input, LParen) {
            let kind = if let Ok(container_condition) = input.try_parse(ContainerCondition::parse) {
                QueryInParensKind::ContainerCondition(container_condition)
            } else {
                QueryInParensKind::SizeFeature(arena_box!(input, input.parse()?))
            };
            let (_, Span { end, .. }) = expect!(input, RParen);
            Ok(QueryInParens { kind, span: Span { start, end } })
        } else {
            let (style_keyword, ident_span) = expect!(input, Ident);
            let keyword = style_keyword.name();
            if keyword.eq_ignore_ascii_case("style") {
                expect_without_ws_or_comments!(input, LParen);
                let kind = input.parse().map(QueryInParensKind::StyleQuery)?;
                let (_, Span { end, .. }) = expect!(input, RParen);
                Ok(QueryInParens { kind, span: Span { start: ident_span.start, end } })
            } else if keyword.eq_ignore_ascii_case("scroll-state") {
                // https://drafts.csswg.org/css-conditional-5/#scroll-state-container
                expect_without_ws_or_comments!(input, LParen);
                // `scroll-state((stuck: top) and (stuck: left))` nests a full
                // condition; `scroll-state(stuck: top)` is a bare feature
                if matches!(peek!(input).token, Token::LParen(..)) {
                    let condition = input.parse()?;
                    let kind =
                        QueryInParensKind::ScrollStateCondition(arena_box!(input, condition));
                    let (_, Span { end, .. }) = expect!(input, RParen);
                    return Ok(QueryInParens { kind, span: Span { start: ident_span.start, end } });
                }
                let media = input.parse()?;
                let kind = QueryInParensKind::ScrollState(arena_box!(input, media));
                let (_, Span { end, .. }) = expect!(input, RParen);
                Ok(QueryInParens { kind, span: Span { start: ident_span.start, end } })
            } else {
                Err(Error { kind: ErrorKind::ExpectStyleQuery, span: ident_span })
            }
        }
    }
}

impl<'a> Parse<'a> for StyleCondition<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match &peek!(input).token {
            Token::Ident(ident) if ident.name().eq_ignore_ascii_case("not") => {
                let style_condition_not = input.parse::<StyleConditionNot>()?;
                let span = style_condition_not.span.clone();
                Ok(StyleCondition {
                    conditions: arena_vec!(input; StyleConditionKind::Not(style_condition_not)),
                    span,
                })
            }
            _ => {
                let first = input.parse::<StyleInParens>()?;
                let mut span = first.span.clone();
                let mut conditions = arena_vec!(input; StyleConditionKind::StyleInParens(first));
                if let Token::Ident(ident) = &peek!(input).token {
                    let name = ident.name();
                    if name.eq_ignore_ascii_case("and") {
                        loop {
                            conditions.push(StyleConditionKind::And(input.parse()?));
                            match &peek!(input).token {
                                Token::Ident(ident) if ident.name().eq_ignore_ascii_case("and") => {
                                }
                                _ => break,
                            }
                        }
                    } else if name.eq_ignore_ascii_case("or") {
                        loop {
                            conditions.push(StyleConditionKind::Or(input.parse()?));
                            match &peek!(input).token {
                                Token::Ident(ident) if ident.name().eq_ignore_ascii_case("or") => {}
                                _ => break,
                            }
                        }
                    }
                }

                if let Some(last) = conditions.last() {
                    span.end = last.span().end;
                }
                Ok(StyleCondition { conditions, span })
            }
        }
    }
}

impl<'a> Parse<'a> for StyleConditionAnd<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let ident = input.parse::<Ident>()?;
        if ident.name.eq_ignore_ascii_case("and") {
            let style_in_parens = input.parse::<StyleInParens>()?;
            let span = Span { start: ident.span.start, end: style_in_parens.span.end };
            Ok(StyleConditionAnd { keyword: ident, style_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectStyleConditionAnd, span: ident.span })
        }
    }
}

impl<'a> Parse<'a> for StyleConditionNot<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let keyword = input.parse::<Ident>()?;
        if keyword.name.eq_ignore_ascii_case("not") {
            let style_in_parens = input.parse::<StyleInParens>()?;
            let span = Span { start: keyword.span.start, end: style_in_parens.span.end };
            Ok(StyleConditionNot { keyword, style_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectStyleConditionNot, span: keyword.span })
        }
    }
}

impl<'a> Parse<'a> for StyleConditionOr<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let keyword = input.parse::<Ident>()?;
        if keyword.name.eq_ignore_ascii_case("or") {
            let style_in_parens = input.parse::<StyleInParens>()?;
            let span = Span { start: keyword.span.start, end: style_in_parens.span.end };
            Ok(StyleConditionOr { keyword, style_in_parens, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectStyleConditionOr, span: keyword.span })
        }
    }
}

impl<'a> Parse<'a> for StyleInParens<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, Span { start, .. }) = expect!(input, LParen);
        let kind = input.parse()?;
        let (_, Span { end, .. }) = expect!(input, RParen);
        Ok(StyleInParens { kind, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for StyleInParensKind<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        if let Ok(style_condition) = input.try_parse(StyleCondition::parse) {
            Ok(StyleInParensKind::Condition(style_condition))
        } else {
            input.parse().map(StyleInParensKind::Feature)
        }
    }
}

impl<'a> Parse<'a> for StyleQuery<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        if let Ok(condition) = input.try_parse(StyleCondition::parse) {
            Ok(StyleQuery::Condition(condition))
        } else if let Ok(name) = input.try_parse(|p| {
            // a bare custom-property existence test: `style(--theme)`
            let name = p.parse::<InterpolableIdent>()?;
            match (&name, &peek!(p).token) {
                (InterpolableIdent::Literal(ident), Token::RParen(..))
                    if ident.name.starts_with("--") =>
                {
                    Ok(name)
                }
                _ => {
                    let span = peek!(p).span.clone();
                    Err(Error { kind: ErrorKind::TryParseError, span })
                }
            }
        }) {
            Ok(StyleQuery::FeatureName(name))
        } else {
            let feature = input.parse().map(StyleQuery::Feature);
            eat!(input, Semicolon);
            feature
        }
    }
}

// https://drafts.csswg.org/css-contain-3/#container-rule
impl<'a> Parse<'a> for ContainerPrelude<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let name = input.try_parse(|parser| match parser.parse()? {
            InterpolableIdent::Literal(ident)
                if ident.name.eq_ignore_ascii_case("not")
                    || ident.name.eq_ignore_ascii_case("scroll-state") =>
            {
                Err(Error { kind: ErrorKind::TryParseError, span: ident.span })
            }
            InterpolableIdent::Literal(ident) if ident.name.eq_ignore_ascii_case("style") => {
                match peek!(parser) {
                    TokenWithSpan { token: Token::LParen(..), span }
                        if span.start == ident.span.end =>
                    {
                        Err(Error { kind: ErrorKind::TryParseError, span: ident.span })
                    }
                    _ => Ok(InterpolableIdent::Literal(ident)),
                }
            }
            ident => Ok(ident),
        });
        // `@container my-page-layout { ... }` — querying by name only
        // (css-conditional-5) has no condition at all.
        if let (Ok(parsed_name), Token::LBrace(..)) = (&name, &peek!(input).token) {
            let span = parsed_name.span().clone();
            return Ok(ContainerPrelude { name: name.ok(), condition: None, span });
        }
        let condition = input.parse::<ContainerCondition>()?;
        let mut span = condition.span().clone();
        if let Ok(name) = &name {
            span.start = name.span().start;
        }
        Ok(ContainerPrelude { name: name.ok(), condition: Some(condition), span })
    }
}

use super::Parser;
use crate::{
    Parse, arena_box, arena_vec,
    ast::*,
    error::{Error, ErrorKind, PResult},
    expect, peek,
    pos::{Span, Spanned},
    tokenizer::{Token, TokenWithSpan},
};

// https://drafts.csswg.org/css-conditional-3/#at-supports
impl<'a> Parse<'a> for SupportsCondition<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match &peek!(input).token {
            Token::Ident(token) if token.name().eq_ignore_ascii_case("not") => {
                let keyword = input.parse::<Ident>()?;
                let condition = input.parse::<SupportsInParens>()?;
                let span = Span { start: keyword.span.start, end: condition.span().end };
                Ok(SupportsCondition {
                    conditions: arena_vec!(input; SupportsConditionKind::Not(SupportsNot {
                        keyword,
                        condition,
                        span: span.clone(),
                    })),
                    span,
                })
            }
            _ => {
                let first = input.parse::<SupportsInParens>()?;
                let mut span = first.span().clone();
                let mut conditions =
                    arena_vec!(input; SupportsConditionKind::SupportsInParens(first));
                while let Token::Ident(ident) = &peek!(input).token {
                    let name = ident.name();
                    if name.eq_ignore_ascii_case("and") {
                        let ident = input.parse::<Ident>()?;
                        let condition = input.parse::<SupportsInParens>()?;
                        let span = Span { start: ident.span.start, end: condition.span().end };
                        conditions.push(SupportsConditionKind::And(SupportsAnd {
                            keyword: ident,
                            condition,
                            span,
                        }));
                    } else if name.eq_ignore_ascii_case("or") {
                        let ident = input.parse::<Ident>()?;
                        let condition = input.parse::<SupportsInParens>()?;
                        let span = Span { start: ident.span.start, end: condition.span().end };
                        conditions.push(SupportsConditionKind::Or(SupportsOr {
                            keyword: ident,
                            condition,
                            span,
                        }));
                    } else {
                        break;
                    }
                }
                if let Some(last) = conditions.last() {
                    span.end = last.span().end;
                }
                Ok(SupportsCondition { conditions, span })
            }
        }
    }
}

impl<'a> Parse<'a> for SupportsInParens<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match peek!(input) {
            TokenWithSpan { token: Token::LParen(..), .. } => input
                .try_parse(|parser| {
                    parser.parse::<SupportsDecl>().map(|supports_decl| {
                        let span = supports_decl.span.clone();
                        SupportsInParens {
                            kind: SupportsInParensKind::Feature(arena_box!(parser, supports_decl)),
                            span,
                        }
                    })
                })
                .or_else(|_| {
                    input.try_parse(|parser| {
                        let (_, Span { start, .. }) = expect!(parser, LParen);
                        let condition = parser.parse::<SupportsCondition>()?;
                        let (_, Span { end, .. }) = expect!(parser, RParen);
                        Ok(SupportsInParens {
                            kind: SupportsInParensKind::SupportsCondition(condition),
                            span: Span { start, end },
                        })
                    })
                })
                .or_else(|_| {
                    // <general-enclosed>: MQ L4 catch-all (referenced from <supports-condition>),
                    // evaluates false at runtime.
                    let (_, Span { start, .. }) = expect!(input, LParen);
                    let tokens = input.parse_tokens_in_parens()?;
                    let (_, Span { end, .. }) = expect!(input, RParen);
                    Ok(SupportsInParens {
                        kind: SupportsInParensKind::GeneralEnclosed(tokens),
                        span: Span { start, end },
                    })
                }),
            // Sass: an interpolation may stand for a whole condition operand
            // (`@supports #{"(a: b)"} and (c: d)`) or splice into a function
            // name (`@supports a#{"b"}c(d)`).
            TokenWithSpan { token: Token::Ident(..) | Token::HashLBrace(..), .. } => {
                let name = input.parse::<InterpolableIdent>()?;
                let name_end = name.span().end;
                match name {
                    InterpolableIdent::Literal(function_ident)
                        if function_ident.name.eq_ignore_ascii_case("selector") =>
                    {
                        expect!(input, LParen);
                        let selector_list = input.parse::<SelectorList>()?;
                        expect!(input, RParen);
                        let span = selector_list.span.clone();
                        Ok(SupportsInParens {
                            kind: SupportsInParensKind::Selector(selector_list),
                            span,
                        })
                    }
                    name => {
                        let glued_lparen = matches!(
                            peek!(input),
                            TokenWithSpan { token: Token::LParen(..), span }
                                if span.start == name_end
                        );
                        // Only a pure interpolation may stand alone
                        // (`#{"(a: b)"}`); a mixed ident like `a#{b}` still
                        // needs parens or a function call, as in dart-sass.
                        let pure_interpolation = matches!(
                            &name,
                            InterpolableIdent::SassInterpolated(interpolation)
                                if matches!(
                                    interpolation.elements.as_slice(),
                                    [SassInterpolatedIdentElement::Expression(..)]
                                )
                        );
                        if glued_lparen {
                            // An unknown function here is `<general-enclosed>`
                            // (css-conditional): its contents are raw tokens.
                            let function = input.parse_raw_function(name)?;
                            let span = function.span.clone();
                            Ok(SupportsInParens {
                                kind: SupportsInParensKind::Function(function),
                                span,
                            })
                        } else if pure_interpolation {
                            let span = name.span().clone();
                            Ok(SupportsInParens {
                                kind: SupportsInParensKind::Interpolation(name),
                                span,
                            })
                        } else {
                            let TokenWithSpan { token, span } = peek!(input);
                            Err(Error {
                                kind: ErrorKind::Unexpected("'('", token.symbol()),
                                span: span.clone(),
                            })
                        }
                    }
                }
            }
            TokenWithSpan { token, span } => Err(Error {
                kind: ErrorKind::Unexpected("'('", token.symbol()),
                span: span.clone(),
            }),
        }
    }
}

impl<'a> Parse<'a> for SupportsDecl<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let start = expect!(input, LParen).1.start;
        let decl = input.parse()?;
        let end = expect!(input, RParen).1.end;
        Ok(SupportsDecl { decl, span: Span { start, end } })
    }
}

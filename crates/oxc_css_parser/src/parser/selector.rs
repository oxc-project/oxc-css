use super::Parser;
use crate::{
    Parse, Syntax, arena_vec,
    ast::*,
    bump, eat,
    error::{Error, ErrorKind, PResult},
    expect, expect_without_ws_or_comments, peek,
    pos::{Span, Spanned},
    tokenizer::{Token, TokenWithSpan, token},
    util,
};

// https://www.w3.org/TR/css-syntax-3/#the-anb-type
impl<'a> Parse<'a> for AnPlusB {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match peek!(input) {
            TokenWithSpan { token: Token::Dimension(..), .. } => {
                let (token::Dimension { value, unit }, span) = expect!(input, Dimension);
                let value_span = Span { start: span.start, end: span.start + value.raw.len() };
                let unit_name = unit.name();
                if unit_name.eq_ignore_ascii_case("n") {
                    match &peek!(input).token {
                        // syntax: <n-dimension> ['+' | '-'] <signless-integer>
                        // examples: '1n + 1', '1n - 1', '1n+ 1'
                        sign @ Token::Plus(..) | sign @ Token::Minus(..) => {
                            let sign = if let Token::Plus(..) = sign { 1 } else { -1 };
                            bump!(input);
                            let (number, number_span) = expect_unsigned_int(input)?;
                            let span = Span { start: span.start, end: number_span.end };
                            Ok(AnPlusB {
                                a: value
                                    .try_into()
                                    .map_err(|kind| Error { kind, span: value_span })?,
                                b: sign
                                    * i32::try_from(number)
                                        .map_err(|kind| Error { kind, span: number_span })?,
                                span,
                            })
                        }

                        // syntax: <n-dimension> <signed-integer>
                        // examples: '1n +1', '1n -1'
                        Token::Number(..) => {
                            let (number, number_span) = expect!(input, Number);
                            let span = Span { start: span.start, end: number_span.end };
                            Ok(AnPlusB {
                                a: value
                                    .try_into()
                                    .map_err(|kind| Error { kind, span: value_span })?,
                                b: number
                                    .try_into()
                                    .map_err(|kind| Error { kind, span: number_span })?,
                                span,
                            })
                        }

                        // syntax: <n-dimension>
                        // examples: '1n'
                        _ => Ok(AnPlusB {
                            a: value.try_into().map_err(|kind| Error { kind, span: value_span })?,
                            b: 0,
                            span,
                        }),
                    }
                } else if unit_name.eq_ignore_ascii_case("n-") {
                    // syntax: <ndash-dimension> <signless-integer>
                    // examples: '1n- 1'
                    let (number, number_span) = expect_unsigned_int(input)?;
                    let span = Span { start: span.start, end: number_span.end };
                    Ok(AnPlusB {
                        a: value.try_into().map_err(|kind| Error { kind, span: value_span })?,
                        b: -i32::try_from(number)
                            .map_err(|kind| Error { kind, span: number_span })?,
                        span,
                    })
                } else if let Some(digits) = unit_name.strip_prefix("n-") {
                    // syntax: <ndashdigit-dimension>
                    // examples: '1n-1'
                    if digits.chars().any(|c| !c.is_ascii_digit()) {
                        return Err(Error {
                            kind: ErrorKind::ExpectUnsignedInteger,
                            span: Span { start: span.start + value.raw.len() + 2, end: span.end },
                        });
                    }
                    let b = digits.parse::<i32>().map_err(|_| Error {
                        kind: ErrorKind::ExpectInteger,
                        span: Span { start: span.start + value.raw.len() + 2, end: span.end },
                    })?;
                    Ok(AnPlusB {
                        a: value.try_into().map_err(|kind| Error { kind, span: value_span })?,
                        b: -b,
                        span,
                    })
                } else {
                    Err(Error { kind: ErrorKind::InvalidAnPlusB, span })
                }
            }

            TokenWithSpan { token: Token::Plus(..), .. } => {
                let plus_span = bump!(input).span;
                let (ident, ident_span) = expect_without_ws_or_comments!(input, Ident);
                let ident_name = ident.name();
                if ident_name.eq_ignore_ascii_case("n") {
                    match &peek!(input).token {
                        // syntax: +n ['+' | '-'] <signless-integer>
                        // examples: '+n + 1', '+n - 1', '+n+ 1'
                        sign @ Token::Plus(..) | sign @ Token::Minus(..) => {
                            let sign = if let Token::Plus(..) = sign { 1 } else { -1 };
                            bump!(input);
                            let (number, number_span) = expect_unsigned_int(input)?;
                            let span = Span { start: plus_span.start, end: number_span.end };
                            Ok(AnPlusB {
                                a: 1,
                                b: sign
                                    * i32::try_from(number)
                                        .map_err(|kind| Error { kind, span: number_span })?,
                                span,
                            })
                        }

                        // syntax: +n <signed-integer>
                        // examples: '+n +1', '+n -1'
                        Token::Number(..) => {
                            let (number, number_span) = expect!(input, Number);
                            let span = Span { start: plus_span.start, end: number_span.end };
                            Ok(AnPlusB {
                                a: 1,
                                b: number
                                    .try_into()
                                    .map_err(|kind| Error { kind, span: number_span })?,
                                span,
                            })
                        }

                        // syntax: +n
                        _ => Ok(AnPlusB {
                            a: 1,
                            b: 0,
                            span: Span { start: plus_span.start, end: ident_span.end },
                        }),
                    }
                } else if ident_name.eq_ignore_ascii_case("n-") {
                    // syntax: +n- <signless-integer>
                    // examples: '+n- 1'
                    let (number, number_span) = expect_unsigned_int(input)?;
                    let span = Span { start: plus_span.start, end: number_span.end };
                    Ok(AnPlusB {
                        a: 1,
                        b: -i32::try_from(number)
                            .map_err(|kind| Error { kind, span: number_span })?,
                        span,
                    })
                } else if let Some(digits) = ident_name.strip_prefix("n-") {
                    // syntax: +<ndashdigit-ident>
                    // examples: '+n-1'
                    if digits.chars().any(|c| !c.is_ascii_digit()) {
                        return Err(Error {
                            kind: ErrorKind::ExpectUnsignedInteger,
                            span: Span { start: ident_span.start + 2, end: ident_span.end },
                        });
                    }
                    let b = digits.parse::<i32>().map_err(|_| Error {
                        kind: ErrorKind::ExpectInteger,
                        span: Span { start: ident_span.start + 2, end: ident_span.end },
                    })?;
                    Ok(AnPlusB {
                        a: 1,
                        b: -b,
                        span: Span { start: plus_span.start, end: ident_span.end },
                    })
                } else {
                    Err(Error {
                        kind: ErrorKind::InvalidAnPlusB,
                        span: Span { start: plus_span.start, end: ident_span.end },
                    })
                }
            }

            TokenWithSpan { token: Token::Ident(..), .. } => {
                let (ident, ident_span) = expect!(input, Ident);
                let ident_name = ident.name();
                if ident_name.eq_ignore_ascii_case("n") {
                    match &peek!(input).token {
                        // syntax: n ['+' | '-'] <signless-integer>
                        // examples: 'n + 1', 'n - 1', 'n+ 1'
                        sign @ Token::Plus(..) | sign @ Token::Minus(..) => {
                            let sign = if let Token::Plus(..) = sign { 1 } else { -1 };
                            bump!(input);
                            let (number, number_span) = expect_unsigned_int(input)?;
                            let span = Span { start: ident_span.start, end: number_span.end };
                            Ok(AnPlusB {
                                a: 1,
                                b: sign
                                    * i32::try_from(number)
                                        .map_err(|kind| Error { kind, span: number_span })?,
                                span,
                            })
                        }

                        // syntax: n <signed-integer>
                        // examples: 'n +1', 'n -1'
                        Token::Number(..) => {
                            let (number, number_span) = expect!(input, Number);
                            let span = Span { start: ident_span.start, end: number_span.end };
                            Ok(AnPlusB {
                                a: 1,
                                b: number
                                    .try_into()
                                    .map_err(|kind| Error { kind, span: number_span })?,
                                span,
                            })
                        }

                        // syntax: n
                        _ => Ok(AnPlusB { a: 1, b: 0, span: ident_span }),
                    }
                } else if ident_name.eq_ignore_ascii_case("n-") {
                    // syntax: n- <signless-integer>
                    // examples: 'n- 1'
                    let (number, number_span) = expect_unsigned_int(input)?;
                    let span = Span { start: ident_span.start, end: number_span.end };
                    Ok(AnPlusB {
                        a: 1,
                        b: -i32::try_from(number)
                            .map_err(|kind| Error { kind, span: number_span })?,
                        span,
                    })
                } else if let Some(digits) = ident_name.strip_prefix("n-") {
                    // syntax: <ndashdigit-ident>
                    // examples: 'n-1'
                    if digits.chars().any(|c| !c.is_ascii_digit()) {
                        return Err(Error {
                            kind: ErrorKind::ExpectUnsignedInteger,
                            span: Span { start: ident_span.start + 2, end: ident_span.end },
                        });
                    }
                    let b = digits.parse::<i32>().map_err(|_| Error {
                        kind: ErrorKind::ExpectInteger,
                        span: Span { start: ident_span.start + 2, end: ident_span.end },
                    })?;
                    Ok(AnPlusB { a: 1, b: -b, span: ident_span })
                } else if ident_name.eq_ignore_ascii_case("-n") {
                    match &peek!(input).token {
                        // syntax: -n ['+' | '-'] <signless-integer>
                        // examples: '-n + 1', '-n - 1', '-n+ 1'
                        sign @ Token::Plus(..) | sign @ Token::Minus(..) => {
                            let sign = if let Token::Plus(..) = sign { 1 } else { -1 };
                            bump!(input);
                            let (number, number_span) = expect_unsigned_int(input)?;
                            let span = Span { start: ident_span.start, end: number_span.end };
                            Ok(AnPlusB {
                                a: -1,
                                b: sign
                                    * i32::try_from(number)
                                        .map_err(|kind| Error { kind, span: number_span })?,
                                span,
                            })
                        }

                        // syntax: -n <signed-integer>
                        // examples: '-n +1', '-n -1'
                        Token::Number(..) => {
                            let (number, number_span) = expect!(input, Number);
                            let span = Span { start: ident_span.start, end: number_span.end };
                            Ok(AnPlusB {
                                a: -1,
                                b: number
                                    .try_into()
                                    .map_err(|kind| Error { kind, span: number_span })?,
                                span,
                            })
                        }

                        // syntax: -n
                        _ => Ok(AnPlusB { a: -1, b: 0, span: ident_span }),
                    }
                } else if ident_name.eq_ignore_ascii_case("-n-") {
                    // syntax: -n- <signless-integer>
                    // examples: '-n- 1'
                    let (number, number_span) = expect_unsigned_int(input)?;
                    let span = Span { start: ident_span.start, end: number_span.end };
                    Ok(AnPlusB {
                        a: -1,
                        b: -i32::try_from(number)
                            .map_err(|kind| Error { kind, span: number_span })?,
                        span,
                    })
                } else if let Some(digits) = ident_name.strip_prefix("-n-") {
                    // syntax: -n-<ndashdigit-ident>
                    // examples: '-n-1'
                    if digits.chars().any(|c| !c.is_ascii_digit()) {
                        return Err(Error {
                            kind: ErrorKind::ExpectUnsignedInteger,
                            span: Span { start: ident_span.start + 3, end: ident_span.end },
                        });
                    }
                    let b = digits.parse::<i32>().map_err(|_| Error {
                        kind: ErrorKind::ExpectInteger,
                        span: Span { start: ident_span.start + 3, end: ident_span.end },
                    })?;
                    Ok(AnPlusB { a: -1, b: -b, span: ident_span })
                } else {
                    Err(Error { kind: ErrorKind::InvalidAnPlusB, span: ident_span })
                }
            }

            TokenWithSpan { span, .. } => {
                Err(Error { kind: ErrorKind::InvalidAnPlusB, span: span.clone() })
            }
        }
    }
}

impl<'a> Parse<'a> for AttributeSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let start = expect!(input, LBracket).1.start;

        let name = match peek!(input) {
            TokenWithSpan {
                token: Token::Ident(..) | Token::HashLBrace(..) | Token::AtLBraceVar(..),
                ..
            } => {
                let ident = input.parse::<InterpolableIdent>()?;
                let ident_span = ident.span();
                if let Some((_, bar_token_span)) = eat!(input, Bar) {
                    let name = input.parse::<InterpolableIdent>()?;
                    let name_span = name.span();

                    let start = ident_span.start;
                    let end = name_span.end;
                    WqName {
                        name,
                        prefix: Some(NsPrefix {
                            kind: Some(NsPrefixKind::Ident(ident)),
                            span: Span { start, end: bar_token_span.end },
                        }),
                        span: Span { start, end },
                    }
                } else {
                    let span = ident_span.clone();
                    WqName { name: ident, prefix: None, span }
                }
            }
            TokenWithSpan { token: Token::Asterisk(..), .. } => {
                let asterisk_span = bump!(input).span;
                let bar_token_span = expect!(input, Bar).1;
                let name = input.parse::<InterpolableIdent>()?;

                let start = asterisk_span.start;
                let end = name.span().end;
                WqName {
                    name,
                    prefix: Some(NsPrefix {
                        kind: Some(NsPrefixKind::Universal(NsPrefixUniversal {
                            span: asterisk_span,
                        })),
                        span: Span { start, end: bar_token_span.end },
                    }),
                    span: Span { start, end },
                }
            }
            TokenWithSpan { token: Token::Bar(..), .. } => {
                let bar_token_span = bump!(input).span;
                let name = input.parse::<InterpolableIdent>()?;

                let start = bar_token_span.start;
                let end = name.span().end;
                WqName {
                    name,
                    prefix: Some(NsPrefix {
                        kind: None,
                        span: Span { start, end: bar_token_span.end },
                    }),
                    span: Span { start, end },
                }
            }
            TokenWithSpan { span, .. } => {
                return Err(Error { kind: ErrorKind::ExpectWqName, span: span.clone() });
            }
        };

        let matcher = match peek!(input) {
            TokenWithSpan { token: Token::RBracket(..), .. } => None,
            TokenWithSpan { token: Token::Equal(..), .. } => Some(AttributeSelectorMatcher {
                kind: AttributeSelectorMatcherKind::Exact,
                span: bump!(input).span,
            }),
            TokenWithSpan { token: Token::TildeEqual(..), .. } => Some(AttributeSelectorMatcher {
                kind: AttributeSelectorMatcherKind::MatchWord,
                span: bump!(input).span,
            }),
            TokenWithSpan { token: Token::BarEqual(..), .. } => Some(AttributeSelectorMatcher {
                kind: AttributeSelectorMatcherKind::ExactOrPrefixThenHyphen,
                span: bump!(input).span,
            }),
            TokenWithSpan { token: Token::CaretEqual(..), .. } => Some(AttributeSelectorMatcher {
                kind: AttributeSelectorMatcherKind::Prefix,
                span: bump!(input).span,
            }),
            TokenWithSpan { token: Token::DollarEqual(..), .. } => Some(AttributeSelectorMatcher {
                kind: AttributeSelectorMatcherKind::Suffix,
                span: bump!(input).span,
            }),
            TokenWithSpan { token: Token::AsteriskEqual(..), .. } => {
                Some(AttributeSelectorMatcher {
                    kind: AttributeSelectorMatcherKind::Substring,
                    span: bump!(input).span,
                })
            }
            TokenWithSpan { span, .. } => {
                return Err(Error {
                    kind: ErrorKind::ExpectAttributeSelectorMatcher,
                    span: span.clone(),
                });
            }
        };

        let value = if matcher.is_some() {
            match peek!(input) {
                TokenWithSpan {
                    token:
                        Token::Ident(..)
                        | Token::HashLBrace(..)
                        | Token::AtLBraceVar(..)
                        | Token::Placeholder(..),
                    ..
                } => Some(AttributeSelectorValue::Ident(input.parse()?)),
                TokenWithSpan { token: Token::Str(..) | Token::StrTemplate(..), .. } => {
                    Some(AttributeSelectorValue::Str(input.parse()?))
                }
                // Unquoted numeric values such as `[size=1]` or `[size=1px]` are
                // technically non-conforming (Selectors wants an ident or string), but
                // browsers accept them and they appear in real CSS (incl. UA stylesheets).
                TokenWithSpan { token: Token::Number(..), .. } => {
                    Some(AttributeSelectorValue::Number(input.parse()?))
                }
                TokenWithSpan { token: Token::Dimension(..), .. } => {
                    Some(AttributeSelectorValue::Dimension(input.parse()?))
                }
                TokenWithSpan { token: Token::Percentage(..), .. }
                    if input.syntax == Syntax::Less =>
                {
                    Some(AttributeSelectorValue::Percentage(input.parse()?))
                }
                TokenWithSpan { token: Token::Tilde(..), .. } if input.syntax == Syntax::Less => {
                    Some(AttributeSelectorValue::LessEscapedStr(input.parse()?))
                }
                TokenWithSpan { token: Token::RBracket(..), span } => {
                    input.recoverable_errors.push(Error {
                        kind: ErrorKind::ExpectAttributeSelectorValue,
                        span: span.clone(),
                    });
                    None
                }
                token_with_span => {
                    return Err(Error {
                        kind: ErrorKind::ExpectAttributeSelectorValue,
                        span: token_with_span.span.clone(),
                    });
                }
            }
        } else {
            None
        };

        let modifier = if value.is_some() {
            match &peek!(input).token {
                Token::Ident(..) | Token::HashLBrace(..) => {
                    let ident = input.parse::<InterpolableIdent>()?;
                    let span = ident.span().clone();
                    Some(AttributeSelectorModifier { ident, span })
                }
                _ => None,
            }
        } else {
            None
        };

        let end = expect!(input, RBracket).1.end;
        Ok(AttributeSelector { name, matcher, value, modifier, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for ClassSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, dot_span) = expect!(input, Dot);
        let start = dot_span.start;
        let end;
        // Detect an adjacent placeholder without `peek!`: `peek!` skips
        // whitespace and caches a token, which would both break the no-ws rule
        // (the name must immediately follow the dot) and trip the empty-cache
        // assertion in the `expect_without_ws_or_comments!` fallback. `scan_placeholder`
        // returns `None` (leaving the tokenizer untouched) unless a placeholder
        // begins exactly here, so the fallback paths run with an empty cache.
        let placeholder = if input.options.template_placeholder.is_some() {
            input.tokenizer.scan_placeholder()
        } else {
            None
        };
        let name = if let Some(TokenWithSpan { token: Token::Placeholder(placeholder), span }) =
            placeholder
        {
            end = span.end;
            InterpolableIdent::Placeholder((placeholder, span).into())
        } else if input.syntax == Syntax::Css {
            let (ident, ident_span) = expect_without_ws_or_comments!(input, Ident);
            end = ident_span.end;
            InterpolableIdent::Literal(input.ident(ident, ident_span))
        } else {
            let ident = input.parse::<InterpolableIdent>()?;
            let ident_span = ident.span();
            util::assert_no_ws_or_comment(&dot_span, ident_span)?;
            end = ident_span.end;
            ident
        };

        Ok(ClassSelector { name, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for ComplexSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let mut children = input.vec_with_capacity(3);

        let (span, first, mut is_previous_combinator) = if let Token::GreaterThan(..)
        | Token::Plus(..)
        | Token::Tilde(..)
        | Token::BarBar(..) = peek!(input).token
        {
            let end = input.tokenizer.current_offset();
            if let Some(combinator) = input.parse_combinator(end)? {
                (combinator.span.clone(), ComplexSelectorChild::Combinator(combinator), true)
            } else {
                return Err(Error {
                    kind: ErrorKind::ExpectSimpleSelector,
                    span: bump!(input).span,
                });
            }
        } else {
            let compound_selector = input.parse::<CompoundSelector>()?;
            (
                compound_selector.span.clone(),
                ComplexSelectorChild::CompoundSelector(compound_selector),
                false,
            )
        };
        let Span { start, mut end } = span;

        children.push(first);
        let is_less = input.syntax == Syntax::Less;
        while !matches!(
            peek!(input).token,
            Token::LBrace(..) | Token::Indent(..) | Token::Linebreak(..)
        ) {
            if is_previous_combinator {
                // dart-sass allows consecutive combinators (`> >`, `+ ~`) and a
                // trailing combinator (`:is(a +)`); after a combinator, take another
                // combinator or stop at a selector boundary rather than requiring a
                // compound selector. CSS keeps the strict alternation.
                if matches!(input.syntax, Syntax::Scss | Syntax::Sass) {
                    if matches!(
                        peek!(input).token,
                        Token::GreaterThan(..)
                            | Token::Plus(..)
                            | Token::Tilde(..)
                            | Token::BarBar(..)
                    ) && let Some(combinator) = input.parse_combinator(end)?
                    {
                        end = combinator.span.end;
                        children.push(ComplexSelectorChild::Combinator(combinator));
                        continue;
                    } else if matches!(
                        peek!(input).token,
                        Token::RParen(..) | Token::Comma(..) | Token::RBrace(..) | Token::Eof(..)
                    ) {
                        break;
                    }
                }
                let compound_selector = input.parse::<CompoundSelector>()?;
                end = compound_selector.span.end;
                children.push(ComplexSelectorChild::CompoundSelector(compound_selector));
            } else if let Some(combinator) = input.parse_combinator(end)? {
                if is_less && combinator.kind == CombinatorKind::Descendant {
                    match &peek!(input).token {
                        Token::Ident(ident) if ident.raw == "when" => break,
                        _ => {}
                    }
                }
                children.push(ComplexSelectorChild::Combinator(combinator));
            } else {
                break;
            }
            is_previous_combinator = !is_previous_combinator;
        }

        Ok(ComplexSelector { children, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for CompoundSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let first = input.parse::<SimpleSelector>()?;
        let first_span = first.span();
        let start = first_span.start;
        let mut end = first_span.end;

        let mut children = input.vec_with_capacity(2);
        children.push(first);
        loop {
            use token::*;
            match peek!(input) {
                TokenWithSpan {
                    token:
                        Token::Dot(..)
                        | Token::Hash(..)
                        | Token::NumberSign(..)
                        | Token::LBracket(..)
                        | Token::Colon(..)
                        | Token::ColonColon(..)
                        | Token::Ident(..)
                        | Token::Asterisk(..)
                        | Token::HashLBrace(..)
                        | Token::Bar(..)
                        | Token::Ampersand(..)
                        | Token::AtLBraceVar(..),
                    span,
                } if !util::has_ws(input.source, end, span.start) => {
                    let child = input.parse::<SimpleSelector>()?;
                    end = child.span().end;
                    children.push(child);
                }
                TokenWithSpan { token: Token::Percent(..), span }
                    if matches!(input.syntax, Syntax::Scss | Syntax::Sass)
                        && !util::has_ws(input.source, end, span.start) =>
                {
                    let child = input.parse::<SimpleSelector>()?;
                    end = child.span().end;
                    children.push(child);
                }
                TokenWithSpan { token: Token::Placeholder(..), span }
                    if !util::has_ws(input.source, end, span.start) =>
                {
                    let child = input.parse::<SimpleSelector>()?;
                    end = child.span().end;
                    children.push(child);
                }
                _ => break,
            }
        }

        Ok(CompoundSelector { children, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for CompoundSelectorList<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let first = input.parse::<CompoundSelector>()?;
        let mut span = first.span.clone();

        let mut selectors = arena_vec!(input; first);
        let mut comma_spans = arena_vec!(input);
        while let Some((_, comma_span)) = eat!(input, Comma) {
            comma_spans.push(comma_span);
            input.eat_sass_line_continuation()?;
            selectors.push(input.parse()?);
        }

        // SAFETY: it has at least one element.
        span.end = unsafe {
            let index = selectors.len() - 1;
            selectors.get_unchecked(index).span().end
        };
        Ok(CompoundSelectorList { selectors, comma_spans, span })
    }
}

impl<'a> Parse<'a> for IdSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match bump!(input) {
            TokenWithSpan { token: Token::Hash(token), span } => {
                let first_span = Span { start: span.start + 1, end: span.end };
                let raw = token.raw;
                if raw.starts_with(|c: char| c.is_ascii_digit())
                    || matches!(raw.as_bytes(), [b'-'] | [b'-', b'0'..=b'9', ..])
                {
                    input
                        .recoverable_errors
                        .push(Error { kind: ErrorKind::InvalidIdSelectorName, span: span.clone() });
                }
                let value = if token.escaped {
                    util::handle_escape_in(raw, input.allocator())
                } else {
                    raw
                };
                let first = Ident { name: value, raw: token.raw, span: first_span };
                let name = match peek!(input) {
                    TokenWithSpan { token: Token::HashLBrace(..), span }
                        if matches!(input.syntax, Syntax::Scss | Syntax::Sass)
                            && first.span.end == span.start =>
                    {
                        match input.parse()? {
                            InterpolableIdent::SassInterpolated(mut interpolation) => {
                                interpolation.elements.insert(
                                    0,
                                    SassInterpolatedIdentElement::Static(
                                        InterpolableIdentStaticPart {
                                            value: first.name,
                                            raw: first.raw,
                                            span: first.span,
                                        },
                                    ),
                                );
                                InterpolableIdent::SassInterpolated(interpolation)
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => InterpolableIdent::Literal(first),
                };
                let span = Span { start: span.start, end: name.span().end };
                Ok(IdSelector { name, span })
            }
            TokenWithSpan { token: Token::NumberSign(..), span } => {
                let name = input.parse::<InterpolableIdent>()?;
                let span = Span { start: span.start, end: name.span().end };
                Ok(IdSelector { name, span })
            }
            TokenWithSpan { span, .. } => Err(Error { kind: ErrorKind::ExpectIdSelector, span }),
        }
    }
}

impl<'a> Parse<'a> for LanguageRange<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match &peek!(input).token {
            Token::Str(..) | Token::StrTemplate(..) => input.parse().map(LanguageRange::Str),
            _ => input.parse().map(LanguageRange::Ident),
        }
    }
}

impl<'a> Parse<'a> for LanguageRangeList<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let first = input.parse::<LanguageRange>()?;
        let mut span = first.span().clone();

        let mut ranges = arena_vec!(input; first);
        let mut comma_spans = arena_vec!(input);
        while let Some((_, comma_span)) = eat!(input, Comma) {
            comma_spans.push(comma_span);
            ranges.push(input.parse()?);
        }
        debug_assert_eq!(comma_spans.len() + 1, ranges.len());

        if let Some(end) = ranges.last() {
            span.end = end.span().end;
        }
        Ok(LanguageRangeList { ranges, comma_spans, span })
    }
}

impl<'a> Parse<'a> for NestingSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, mut span) = expect!(input, Ampersand);
        let suffix = match input.syntax {
            Syntax::Css => {
                if let Some((ident, ident_span)) = input.tokenizer.scan_ident_template()? {
                    span.end = ident_span.end;
                    Some(InterpolableIdent::Literal(input.ident(ident, ident_span)))
                } else {
                    None
                }
            }
            Syntax::Scss | Syntax::Sass => {
                let start = span.end;
                let elements = input.parse_sass_interpolated_ident_rest(&mut span.end)?;
                if elements.is_empty() {
                    None
                } else {
                    Some(InterpolableIdent::SassInterpolated(SassInterpolatedIdent {
                        elements,
                        span: Span { start, end: span.end },
                    }))
                }
            }
            Syntax::Less => {
                let start = span.end;
                let elements = input.parse_less_interpolated_ident_rest(&mut span.end)?;
                if elements.is_empty() {
                    None
                } else {
                    Some(InterpolableIdent::LessInterpolated(LessInterpolatedIdent {
                        elements,
                        span: Span { start, end: span.end },
                    }))
                }
            }
        };
        Ok(NestingSelector { suffix, span })
    }
}

// https://drafts.csswg.org/selectors-4/#the-nth-child-pseudo
impl<'a> Parse<'a> for Nth<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let index = input.parse::<NthIndex>()?;
        let mut span = index.span().clone();
        let matcher = match &peek!(input).token {
            Token::Ident(ident) if ident.name().eq_ignore_ascii_case("of") => {
                let matcher = input.parse::<NthMatcher>()?;
                span.end = matcher.span.end;
                Some(matcher)
            }
            _ => None,
        };

        Ok(Nth { index, matcher, span })
    }
}

impl<'a> Parse<'a> for NthIndex<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match &peek!(input).token {
            Token::Ident(ident) => {
                let name = ident.name();
                if name.eq_ignore_ascii_case("odd") {
                    input.parse().map(NthIndex::Odd)
                } else if name.eq_ignore_ascii_case("even") {
                    input.parse().map(NthIndex::Even)
                } else {
                    input.parse().map(NthIndex::AnPlusB)
                }
            }
            Token::Number(..) => {
                let number = input.parse::<Number>()?;
                if number.value.fract() == 0.0 {
                    Ok(NthIndex::Integer(number))
                } else {
                    Err(Error { kind: ErrorKind::ExpectInteger, span: number.span })
                }
            }
            _ => input.parse().map(NthIndex::AnPlusB),
        }
    }
}

impl<'a> Parse<'a> for NthMatcher<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (ident, mut span) = expect!(input, Ident);
        if !ident.name().eq_ignore_ascii_case("of") {
            return Err(Error { kind: ErrorKind::ExpectNthOf, span });
        }

        let selector = if matches!(&peek!(input).token, Token::RParen(..)) {
            None
        } else {
            let selector = input.parse::<SelectorList>()?;
            span.end = selector.span.end;
            Some(selector)
        };

        Ok(NthMatcher { selector, span })
    }
}

impl<'a> Parse<'a> for PseudoClassSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, colon_span) = expect!(input, Colon);
        let name = input.parse::<InterpolableIdent>()?;
        let name_span = name.span();
        util::assert_no_ws(input.source, &colon_span, name_span)?;

        let mut end = name_span.end;

        let arg = match peek!(input) {
            TokenWithSpan { token: Token::LParen(..), span: l_paren } if l_paren.start == end => {
                let l_paren = l_paren.clone();
                bump!(input);
                let kind = match &name {
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("nth-child")
                            || name.eq_ignore_ascii_case("nth-last-child") =>
                    {
                        if input.syntax == Syntax::Css {
                            input.parse().map(PseudoClassSelectorArgKind::Nth)?
                        } else if let Ok(nth) = input.try_parse(Nth::parse) {
                            PseudoClassSelectorArgKind::Nth(nth)
                        } else {
                            input
                                .parse_tokens_in_parens()
                                .map(PseudoClassSelectorArgKind::TokenSeq)?
                        }
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("nth-of-type")
                            || name.eq_ignore_ascii_case("nth-last-of-type")
                            || name.eq_ignore_ascii_case("nth-col")
                            || name.eq_ignore_ascii_case("nth-last-col") =>
                    'pseudo_arg: {
                        let nth = if input.syntax == Syntax::Css {
                            input.parse()?
                        } else if let Ok(nth) = input.try_parse(Nth::parse) {
                            nth
                        } else {
                            break 'pseudo_arg input
                                .parse_tokens_in_parens()
                                .map(PseudoClassSelectorArgKind::TokenSeq)?;
                        };
                        if let Some(NthMatcher { span, .. }) = &nth.matcher {
                            input.recoverable_errors.push(Error {
                                kind: ErrorKind::UnexpectedNthMatcher,
                                span: span.clone(),
                            });
                        }
                        PseudoClassSelectorArgKind::Nth(nth)
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("not")
                            || name.eq_ignore_ascii_case("is")
                            || name.eq_ignore_ascii_case("where")
                            || name.eq_ignore_ascii_case("matches")
                            || name.eq_ignore_ascii_case("global") =>
                    {
                        input.parse().map(PseudoClassSelectorArgKind::SelectorList)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("has") =>
                    {
                        input.parse().map(PseudoClassSelectorArgKind::RelativeSelectorList)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("dir") =>
                    {
                        input.parse().map(PseudoClassSelectorArgKind::Ident)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("lang") =>
                    {
                        input.parse().map(PseudoClassSelectorArgKind::LanguageRangeList)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("-moz-any")
                            || name.eq_ignore_ascii_case("-webkit-any")
                            || name.eq_ignore_ascii_case("any") =>
                    {
                        // formally compound selectors, but real-world usage
                        // includes complex ones (`:-moz-any(ol p.blah, ul)`)
                        input.parse().map(PseudoClassSelectorArgKind::SelectorList)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("current")
                            || name.eq_ignore_ascii_case("past")
                            || name.eq_ignore_ascii_case("future") =>
                    {
                        input.parse().map(PseudoClassSelectorArgKind::CompoundSelectorList)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("host")
                            || name.eq_ignore_ascii_case("host-context") =>
                    {
                        input.parse().map(PseudoClassSelectorArgKind::CompoundSelector)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if input.syntax == Syntax::Less && *name == "extend" =>
                    {
                        input.parse().map(PseudoClassSelectorArgKind::LessExtendList)?
                    }
                    _ => {
                        input.parse_tokens_in_parens().map(PseudoClassSelectorArgKind::TokenSeq)?
                    }
                };

                let r_paren = expect!(input, RParen).1;
                end = r_paren.end;
                let span = Span { start: l_paren.start, end: r_paren.end };
                Some(PseudoClassSelectorArg { kind, l_paren, r_paren, span })
            }
            _ => None,
        };

        let span = Span { start: colon_span.start, end };
        Ok(PseudoClassSelector { name, arg, span })
    }
}

impl<'a> Parse<'a> for PseudoElementSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, colon_colon_span) = expect!(input, ColonColon);
        let mut end;
        let name = if input.syntax == Syntax::Css {
            let (ident, ident_span) = expect!(input, Ident);
            end = ident_span.end;
            util::assert_no_ws(input.source, &colon_colon_span, &ident_span)?;
            InterpolableIdent::Literal(input.ident(ident, ident_span))
        } else {
            let name = input.parse::<InterpolableIdent>()?;
            let name_span = name.span();
            end = name_span.end;
            util::assert_no_ws(input.source, &colon_colon_span, name_span)?;
            name
        };

        let arg = match peek!(input) {
            TokenWithSpan { token: Token::LParen(..), span: l_paren } if l_paren.start == end => {
                let l_paren = l_paren.clone();
                bump!(input);
                let kind = match &name {
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("part") =>
                    {
                        input.parse().map(PseudoElementSelectorArgKind::Ident)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("cue")
                            || name.eq_ignore_ascii_case("cue-region") =>
                    {
                        input.parse().map(PseudoElementSelectorArgKind::CompoundSelector)?
                    }
                    InterpolableIdent::Literal(Ident { name, .. })
                        if name.eq_ignore_ascii_case("slotted") =>
                    {
                        // formally a single compound selector, but sass extend
                        // output produces lists (`::slotted(.c.d, .d.e)`)
                        input.parse().map(PseudoElementSelectorArgKind::CompoundSelectorList)?
                    }
                    _ => input
                        .parse_tokens_in_parens()
                        .map(PseudoElementSelectorArgKind::TokenSeq)?,
                };

                let r_paren = expect!(input, RParen).1;
                end = r_paren.end;
                let span = Span { start: l_paren.start, end: r_paren.end };
                Some(PseudoElementSelectorArg { kind, l_paren, r_paren, span })
            }
            _ => None,
        };

        let span = Span { start: colon_colon_span.start, end };
        Ok(PseudoElementSelector { name, arg, span })
    }
}

impl<'a> Parse<'a> for RelativeSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let pos = input.tokenizer.current_offset();
        let combinator = match input.parse_combinator(pos)? {
            Some(Combinator { kind: CombinatorKind::Descendant, .. }) => None,
            combinator => combinator,
        };
        let complex_selector = input.parse::<ComplexSelector>()?;
        let mut span = complex_selector.span.clone();
        if let Some(combinator) = &combinator {
            span.start = combinator.span.start;
        }
        Ok(RelativeSelector { combinator, complex_selector, span })
    }
}

impl<'a> Parse<'a> for RelativeSelectorList<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let first = input.parse::<RelativeSelector>()?;
        let mut span = first.span.clone();

        let mut selectors = arena_vec!(input; first);
        let mut comma_spans = arena_vec!(input);
        while let Some((_, comma_span)) = eat!(input, Comma) {
            comma_spans.push(comma_span);
            selectors.push(input.parse()?);
        }

        // SAFETY: it has at least one element.
        span.end = unsafe {
            let index = selectors.len() - 1;
            selectors.get_unchecked(index).span().end
        };
        Ok(RelativeSelectorList { selectors, comma_spans, span })
    }
}

impl<'a> Parse<'a> for SelectorList<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let first = input.parse::<ComplexSelector>()?;
        let mut span = first.span.clone();

        let mut selectors = input.vec_with_capacity(2);
        selectors.push(first);
        let mut comma_spans = arena_vec!(input);

        let is_css = input.syntax == Syntax::Css;
        while let Some((_, comma_span)) = eat!(input, Comma) {
            span.end = comma_span.end;
            comma_spans.push(comma_span);
            // legacy corpora carry doubled/trailing commas (`div,, span,, {`);
            // absorb the extras in SCSS like libsass did
            if input.syntax == Syntax::Scss {
                while let Some((_, comma_span)) = eat!(input, Comma) {
                    span.end = comma_span.end;
                    comma_spans.push(comma_span);
                }
            }
            // In the indented syntax a deeper line after the comma continues
            // the selector list (`a,\n    b\n  c: d`); a same-level line or
            // `{` means the comma was trailing.
            if input.syntax == Syntax::Sass && matches!(peek!(input).token, Token::Indent(..)) {
                input.eat_sass_line_continuation()?;
            } else if !is_css
                && matches!(
                    peek!(input).token,
                    Token::LBrace(..) | Token::Indent(..) | Token::Linebreak(..)
                )
            {
                break;
            }

            let selector = input.parse::<ComplexSelector>()?;
            span.end = selector.span.end;
            selectors.push(selector);
        }

        // absorbed doubled/trailing commas can outnumber the selectors, so
        // phrase the invariants without subtraction (usize underflow)
        debug_assert!(if is_css {
            selectors.len() == comma_spans.len() + 1
        } else {
            selectors.len() <= comma_spans.len() + 1
        });

        Ok(SelectorList { selectors, comma_spans, span })
    }
}

// https://www.w3.org/TR/selectors-4/#ref-for-typedef-simple-selector
impl<'a> Parse<'a> for SimpleSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match peek!(input) {
            TokenWithSpan { token: Token::Dot(..), .. } => input.parse().map(SimpleSelector::Class),
            TokenWithSpan { token: Token::Hash(..) | Token::NumberSign(..), .. } => {
                input.parse().map(SimpleSelector::Id)
            }
            TokenWithSpan { token: Token::LBracket(..), .. } => {
                input.parse().map(SimpleSelector::Attribute)
            }
            TokenWithSpan { token: Token::Colon(..), .. } => {
                input.parse().map(SimpleSelector::PseudoClass)
            }
            TokenWithSpan { token: Token::ColonColon(..), .. } => {
                input.parse().map(SimpleSelector::PseudoElement)
            }
            TokenWithSpan {
                token:
                    Token::Ident(..)
                    | Token::Asterisk(..)
                    | Token::HashLBrace(..)
                    | Token::Bar(..)
                    | Token::AtLBraceVar(..),
                ..
            } => input.parse().map(SimpleSelector::Type),
            TokenWithSpan { token: Token::Ampersand(..), .. } => {
                input.parse().map(SimpleSelector::Nesting)
            }
            TokenWithSpan { token: Token::Percent(..), .. }
                if matches!(input.syntax, Syntax::Scss | Syntax::Sass) =>
            {
                input.parse().map(SimpleSelector::SassPlaceholder)
            }
            TokenWithSpan { token: Token::Placeholder(..), .. } => {
                let name = input.parse::<InterpolableIdent>()?;
                let span = name.span().clone();
                Ok(SimpleSelector::Type(TypeSelector::TagName(TagNameSelector {
                    name: WqName { name, prefix: None, span: span.clone() },
                    span,
                })))
            }
            token_with_span => Err(Error {
                kind: ErrorKind::ExpectSimpleSelector,
                span: token_with_span.span.clone(),
            }),
        }
    }
}

impl<'a> Parse<'a> for TypeSelector<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        enum IdentOrAsterisk<'a> {
            Ident(InterpolableIdent<'a>),
            Asterisk(Span),
        }

        let ident_or_asterisk = match &peek!(input).token {
            Token::Ident(..) | Token::HashLBrace(..) | Token::AtLBraceVar(..) => {
                input.parse().map(IdentOrAsterisk::Ident).map(Some)?
            }
            Token::Asterisk(..) => Some(IdentOrAsterisk::Asterisk(bump!(input).span)),
            Token::Bar(..) => None,
            _ => unreachable!(),
        };

        match peek!(input) {
            TokenWithSpan { token: Token::Bar(..), span }
                if ident_or_asterisk
                    .as_ref()
                    .map(|t| match t {
                        IdentOrAsterisk::Ident(ident) => {
                            !util::has_ws(input.source, ident.span().end, span.start)
                        }
                        IdentOrAsterisk::Asterisk(asterisk_span) => {
                            !util::has_ws(input.source, asterisk_span.end, span.start)
                        }
                    })
                    .unwrap_or(true) =>
            {
                let bar_token_span = bump!(input).span;

                let prefix = match ident_or_asterisk {
                    Some(IdentOrAsterisk::Ident(ident)) => {
                        let mut span = ident.span().clone();
                        span.end = bar_token_span.end;
                        NsPrefix { kind: Some(NsPrefixKind::Ident(ident)), span }
                    }
                    Some(IdentOrAsterisk::Asterisk(asterisk_span)) => {
                        let mut span = asterisk_span.clone();
                        span.end = bar_token_span.end;
                        NsPrefix {
                            kind: Some(NsPrefixKind::Universal(NsPrefixUniversal {
                                span: asterisk_span,
                            })),
                            span,
                        }
                    }
                    None => NsPrefix { kind: None, span: bar_token_span },
                };

                match peek!(input) {
                    TokenWithSpan { token: Token::Ident(..) | Token::HashLBrace(..), .. } => {
                        let name = input.parse::<InterpolableIdent>()?;
                        let name_span = name.span();
                        util::assert_no_ws(input.source, &prefix.span, name_span)?;
                        let span = Span { start: prefix.span.start, end: name_span.end };
                        Ok(TypeSelector::TagName(TagNameSelector {
                            name: WqName { name, prefix: Some(prefix), span: span.clone() },
                            span,
                        }))
                    }
                    TokenWithSpan { token: Token::Asterisk(..), .. } => {
                        let asterisk_span = bump!(input).span;
                        util::assert_no_ws(input.source, &prefix.span, &asterisk_span)?;
                        let span = Span { start: prefix.span.start, end: asterisk_span.end };
                        Ok(TypeSelector::Universal(UniversalSelector {
                            prefix: Some(prefix),
                            span,
                        }))
                    }
                    TokenWithSpan { span, .. } => {
                        Err(Error { kind: ErrorKind::ExpectTypeSelector, span: span.clone() })
                    }
                }
            }

            _ => match ident_or_asterisk {
                Some(IdentOrAsterisk::Ident(ident)) => {
                    let span = ident.span().clone();
                    Ok(TypeSelector::TagName(TagNameSelector {
                        name: WqName { name: ident, prefix: None, span: span.clone() },
                        span,
                    }))
                }
                Some(IdentOrAsterisk::Asterisk(span)) => {
                    Ok(TypeSelector::Universal(UniversalSelector { prefix: None, span }))
                }
                None => unreachable!(),
            },
        }
    }
}

impl<'a> Parser<'a> {
    fn parse_combinator(&mut self, pos: usize) -> PResult<Option<Combinator>> {
        match peek!(self) {
            TokenWithSpan {
                token:
                    Token::Ident(..)
                    | Token::Dot(..)
                    | Token::Hash(..)
                    | Token::Colon(..)
                    | Token::ColonColon(..)
                    | Token::LBracket(..)
                    | Token::Asterisk(..)
                    | Token::Ampersand(..)
                    | Token::Bar(..) // selector like `|type` (with <ns-prefix>)
                    | Token::AtLBraceVar(..)
                    | Token::NumberSign(..)
                    | Token::HashLBrace(..)
                    | Token::Percent(..) // Sass `%placeholder` descendant
                    | Token::Placeholder(..), // `${a} ${b}` descendant
                span,
            } if pos < span.start => Ok(Some(Combinator {
                kind: CombinatorKind::Descendant,
                span: Span {
                    start: pos,
                    end: span.start,
                },
            })),
            TokenWithSpan {
                token: Token::GreaterThan(..),
                ..
            } => Ok(Some(Combinator {
                kind: CombinatorKind::Child,
                span: bump!(self).span,
            })),
            TokenWithSpan {
                token: Token::Plus(..),
                ..
            } => Ok(Some(Combinator {
                kind: CombinatorKind::NextSibling,
                span: bump!(self).span,
            })),
            TokenWithSpan {
                token: Token::Tilde(..),
                ..
            } => Ok(Some(Combinator {
                kind: CombinatorKind::LaterSibling,
                span: bump!(self).span,
            })),
            TokenWithSpan {
                token: Token::BarBar(..),
                ..
            } => Ok(Some(Combinator {
                kind: CombinatorKind::Column,
                span: bump!(self).span,
            })),
            // deprecated shadow-piercing `/deep/` and less.js's arbitrary
            // slashed combinators (`.container /shadow/ .content`) — but not
            // in Scss/Sass, where dart-sass rejects reference combinators
            TokenWithSpan { token: Token::Solidus(..), .. }
                if !matches!(self.syntax, Syntax::Scss | Syntax::Sass) =>
            {
                let deep = self.try_parse(|p| {
                    let start = bump!(p).span; // `/`
                    let ident_end = match peek!(p) {
                        TokenWithSpan { token: Token::Ident(..), span }
                            if span.start == start.end =>
                        {
                            bump!(p).span.end
                        }
                        TokenWithSpan { span, .. } => {
                            return Err(Error {
                                kind: ErrorKind::TryParseError,
                                span: span.clone(),
                            });
                        }
                    };
                    match peek!(p) {
                        TokenWithSpan { token: Token::Solidus(..), span }
                            if span.start == ident_end =>
                        {
                            let end = bump!(p).span.end;
                            Ok(Span { start: start.start, end })
                        }
                        TokenWithSpan { span, .. } => Err(Error {
                            kind: ErrorKind::TryParseError,
                            span: span.clone(),
                        }),
                    }
                });
                match deep {
                    Ok(span) => Ok(Some(Combinator { kind: CombinatorKind::Deep, span })),
                    Err(_) => Ok(None),
                }
            }
            // deprecated shadow combinators `^` and `^^` (Less corpora and
            // the CSS files Less emits)
            TokenWithSpan {
                token: Token::Unknown(..),
                span,
            } if !matches!(self.syntax, Syntax::Scss | Syntax::Sass)
                && self.source.as_bytes().get(span.start) == Some(&b'^') =>
            {
                let start = bump!(self).span.start;
                if matches!(&peek!(self).token, Token::Unknown(..))
                    && peek!(self).span.start == start + 1
                    && self.source.as_bytes().get(start + 1) == Some(&b'^')
                {
                    let end = bump!(self).span.end;
                    Ok(Some(Combinator {
                        kind: CombinatorKind::ShadowDescendant,
                        span: Span { start, end },
                    }))
                } else {
                    Ok(Some(Combinator {
                        kind: CombinatorKind::ShadowChild,
                        span: Span { start, end: start + 1 },
                    }))
                }
            }
            _ => Ok(None),
        }
    }
}

fn expect_unsigned_int<'a>(input: &mut Parser<'a>) -> PResult<(token::Number<'a>, Span)> {
    let (number, span) = expect!(input, Number);
    if number.raw.chars().any(|c| !c.is_ascii_digit()) {
        Err(Error { kind: ErrorKind::ExpectUnsignedInteger, span })
    } else {
        Ok((number, span))
    }
}

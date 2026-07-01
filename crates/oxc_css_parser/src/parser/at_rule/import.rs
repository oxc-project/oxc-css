use super::Parser;
use crate::{
    Parse, Syntax, arena_vec,
    ast::*,
    bump,
    error::{Error, ErrorKind, PResult},
    expect, expect_without_ws_or_comments, peek,
    pos::{Span, Spanned},
    tokenizer::{Token, TokenWithSpan},
};

// https://www.w3.org/TR/css-cascade-5/#at-import
impl<'a> Parse<'a> for ImportPrelude<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let href = match &peek!(input).token {
            Token::Str(..) | Token::StrTemplate(..) => input.parse().map(ImportPreludeHref::Str)?,
            _ => match input.try_parse(Url::parse) {
                Ok(url) => ImportPreludeHref::Url(url),
                // Sass only: the content of `url(...)` may be SassScript that
                // is not a parsable URL, e.g. `@import url($dir+"/path");`.
                // Mirrors the fallback in `parse_component_value_atom`.
                Err(error) if matches!(input.syntax, Syntax::Scss | Syntax::Sass) => {
                    let (function_name, function_name_span) = expect!(input, Ident);
                    let function_name = input.ident(function_name, function_name_span);
                    if !function_name.name.eq_ignore_ascii_case("url") {
                        return Err(error);
                    }
                    input
                        .parse_function(InterpolableIdent::Literal(function_name))
                        .map(ImportPreludeHref::Function)
                        .map_err(|_| error)?
                }
                Err(error) => return Err(error),
            },
        };
        let mut span = href.span().clone();

        let layer = match &peek!(input).token {
            Token::Ident(ident) if ident.name().eq_ignore_ascii_case("layer") => {
                let ident = input.parse::<Ident>()?;
                let layer = match peek!(input) {
                    TokenWithSpan { token: Token::LParen(..), span }
                        if span.start == ident.span.end =>
                    {
                        bump!(input);
                        let args = arena_vec!(input; input.parse().map(ComponentValue::LayerName)?);
                        let end = expect!(input, RParen).1.end;
                        let span = Span { start: ident.span.start, end };
                        ImportPreludeLayer::WithName(Function {
                            name: FunctionName::Ident(InterpolableIdent::Literal(ident)),
                            args,
                            span,
                        })
                    }
                    _ => ImportPreludeLayer::Empty(ident),
                };
                span.end = layer.span().end;
                Some(layer)
            }
            _ => None,
        };

        let supports = input.try_parse(|parser| {
            let (ident, span) = expect!(parser, Ident);
            if !ident.name().eq_ignore_ascii_case("supports") {
                return Err(Error { kind: ErrorKind::TryParseError, span });
            }

            expect_without_ws_or_comments!(parser, LParen);

            let kind = if let Ok(supports_condition) = parser.try_parse(SupportsCondition::parse) {
                ImportPreludeSupportsKind::SupportsCondition(supports_condition)
            } else {
                parser.parse().map(ImportPreludeSupportsKind::Declaration)?
            };
            let (_, Span { end, .. }) = expect!(parser, RParen);
            Ok(ImportPreludeSupports { kind, span: Span { start: span.start, end } })
        });
        if let Ok(supports) = &supports {
            span.end = supports.span().end;
        }

        // `}` ends the at-rule too, so an `@import` nested in a style rule needs no
        // trailing `;` (`a { @import "b.css" }`); it can't start a media query.
        let media = if matches!(
            peek!(input).token,
            Token::Semicolon(..) | Token::Eof(..) | Token::RBrace(..)
        ) {
            None
        } else {
            let media = input.parse::<MediaQueryList>()?;
            span.end = media.span.end;
            Some(media)
        };

        Ok(ImportPrelude { href, layer, supports: supports.ok(), media, span })
    }
}

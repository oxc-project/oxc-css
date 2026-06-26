use super::Parser;
use crate::{Parse, Spanned, ast::*, error::PResult, peek, tokenizer::Token};

// https://www.w3.org/TR/css-namespaces-3/#syntax
impl<'a> Parse<'a> for NamespacePrelude<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let prefix = match &peek!(input).token {
            Token::Ident(ident) => {
                if ident.name().eq_ignore_ascii_case("url") {
                    None
                } else {
                    Some(InterpolableIdent::Literal(input.parse::<Ident>()?))
                }
            }
            Token::HashLBrace(..) | Token::AtLBraceVar(..) => {
                input.parse::<InterpolableIdent>().map(Some)?
            }
            _ => None,
        };
        let uri = match &peek!(input).token {
            Token::Str(..) | Token::StrTemplate(..) => {
                input.parse().map(NamespacePreludeUri::Str)?
            }
            _ => input.parse().map(NamespacePreludeUri::Url)?,
        };

        let mut span = uri.span().clone();
        if let Some(prefix) = &prefix {
            span.start = prefix.span().start;
        }
        Ok(NamespacePrelude { prefix, uri, span })
    }
}

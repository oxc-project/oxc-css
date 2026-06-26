use super::Parser;
use crate::{Parse, Spanned, arena_vec, ast::*, error::PResult, peek, tokenizer::Token};

// https://drafts.csswg.org/css-fonts/Overview.bs
impl<'a> Parse<'a> for FontFamilyName<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match &peek!(input).token {
            Token::Str(..) | Token::StrTemplate(..) => input.parse().map(FontFamilyName::Str),
            _ => {
                let first = input.parse::<InterpolableIdent>()?;
                let mut span = first.span().clone();

                let mut idents = arena_vec!(input; first);
                while let Token::Ident(..) | Token::HashLBrace(..) | Token::AtLBraceVar(..) =
                    &peek!(input).token
                {
                    idents.push(input.parse()?);
                }
                if let Some(last) = idents.last() {
                    span.end = last.span().end;
                }
                Ok(FontFamilyName::Unquoted(UnquotedFontFamilyName { idents, span }))
            }
        }
    }
}

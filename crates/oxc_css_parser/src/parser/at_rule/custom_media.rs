use super::Parser;
use crate::{
    Parse,
    ast::*,
    error::PResult,
    peek,
    pos::{Span, Spanned},
    tokenizer::Token,
};

// https://www.w3.org/TR/mediaqueries-5/#custom-mq
impl<'a> Parse<'a> for CustomMedia<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let name = input.parse_dashed_ident()?;
        let value = input.parse::<CustomMediaValue>()?;
        let span = Span { start: name.span().start, end: value.span().end };
        Ok(CustomMedia { name, value, span })
    }
}

impl<'a> Parse<'a> for CustomMediaValue<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match &peek!(input).token {
            Token::Ident(ident) => {
                let name = ident.name();
                if name.eq_ignore_ascii_case("true") {
                    input.parse().map(CustomMediaValue::True)
                } else if name.eq_ignore_ascii_case("false") {
                    input.parse().map(CustomMediaValue::False)
                } else {
                    input.parse().map(CustomMediaValue::MediaQueryList)
                }
            }
            _ => input.parse().map(CustomMediaValue::MediaQueryList),
        }
    }
}

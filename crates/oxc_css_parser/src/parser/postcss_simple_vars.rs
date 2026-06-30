use super::Parser;
use crate::{
    Parse,
    ast::*,
    config::Syntax,
    error::PResult,
    expect, peek,
    pos::{Span, Spanned},
    tokenizer::Token,
};

impl<'a> Parse<'a> for PostcssSimpleVar<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert!(input.syntax == Syntax::Css && input.options.allow_postcss_simple_vars);

        let (name, span) = input.parse_dollar_var_ident()?;
        Ok(PostcssSimpleVar { name, span })
    }
}

impl<'a> Parse<'a> for PostcssSimpleVarDeclaration<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert!(input.syntax == Syntax::Css && input.options.allow_postcss_simple_vars);

        let name = input.parse::<PostcssSimpleVar>()?;
        let (_, colon_span) = expect!(input, Colon);
        let mut value = input.parse_declaration_value()?;
        // postcss-simple-vars is textual substitution; `!important` is just part
        // of the value, not a structural declaration modifier (unlike CSS's
        // `Declaration.important`). Keep it in the value stream.
        if let Token::Exclamation(..) = &peek!(input).token {
            let important = input.parse::<ImportantAnnotation>()?;
            value.push(ComponentValue::ImportantAnnotation(important));
        }

        let end = value.last().map(|v| v.span().end).unwrap_or(colon_span.end);
        let span = Span { start: name.span.start, end };

        Ok(PostcssSimpleVarDeclaration { name, colon_span, value, span })
    }
}

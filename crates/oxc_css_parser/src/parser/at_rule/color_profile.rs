use super::Parser;
use crate::{Parse, ast::*, error::PResult};

// https://www.w3.org/TR/css-color-5/#at-profile
impl<'a> Parse<'a> for ColorProfilePrelude<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match input.parse_dashed_ident()? {
            InterpolableIdent::Literal(ident) if ident.name.eq_ignore_ascii_case("device-cmyk") => {
                Ok(ColorProfilePrelude::DeviceCmyk(ident))
            }
            ident => Ok(ColorProfilePrelude::DashedIdent(ident)),
        }
    }
}

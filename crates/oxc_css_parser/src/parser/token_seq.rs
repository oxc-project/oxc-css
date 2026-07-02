use super::Parser;
use crate::{
    Syntax,
    ast::*,
    bump,
    error::{Error, ErrorKind, PResult},
    peek,
    pos::Span,
    tokenizer::Token,
    util::PairedToken,
};

impl<'a> Parser<'a> {
    pub(super) fn parse_tokens_in_parens(&mut self) -> PResult<TokenSeq<'a>> {
        let start = self.tokenizer.current_offset();
        let mut tokens = self.vec_with_capacity(1);
        let mut pairs = Vec::with_capacity(1);
        loop {
            match &peek!(self).token {
                // A stray delimiter is a plain token in CSS, but the
                // preprocessor dialects give it real syntax (`$var`, Less
                // `^`), and their reference compilers reject it here.
                Token::Unknown(..) if self.syntax != Syntax::Css => {
                    let span = peek!(self).span.clone();
                    return Err(Error { kind: ErrorKind::UnknownToken, span });
                }
                Token::LParen(..) => {
                    pairs.push(PairedToken::Paren);
                }
                Token::RParen(..) => {
                    if let Some(PairedToken::Paren) = pairs.pop() {
                    } else {
                        break;
                    }
                }
                Token::LBracket(..) => {
                    pairs.push(PairedToken::Bracket);
                }
                Token::RBracket(..) => {
                    if let Some(PairedToken::Bracket) = pairs.pop() {
                    } else {
                        break;
                    }
                }
                Token::LBrace(..) | Token::HashLBrace(..) => {
                    pairs.push(PairedToken::Brace);
                }
                Token::RBrace(..) => {
                    if let Some(PairedToken::Brace) = pairs.pop() {
                    } else {
                        break;
                    }
                }
                Token::Eof(..) => break,
                _ => {}
            }
            tokens.push(bump!(self));
        }
        let span = Span {
            start: tokens.first().map(|token| token.span.start).unwrap_or(start),
            end: if let Some(last) = tokens.last() {
                last.span.end
            } else {
                peek!(self).span.start
            },
        };
        Ok(TokenSeq { tokens, span })
    }
}

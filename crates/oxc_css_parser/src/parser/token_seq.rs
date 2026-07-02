use super::Parser;
use crate::{
    Syntax,
    ast::*,
    bump,
    error::{Error, ErrorKind, PResult},
    peek,
    pos::Span,
    tokenizer::{Token, TokenWithSpan},
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
                // An interpolated string (`("min-width:#{$foo}")`) must be
                // consumed structurally — the tokenizer resumes the string
                // after each `#{...}` — but its pieces are still plain tokens.
                Token::StrTemplate(..) => {
                    self.consume_str_template_tokens_into(&mut tokens)?;
                    continue;
                }
                Token::Eof(..) => break,
                token => {
                    if !crate::util::track_paired_token(token, &mut pairs) {
                        break;
                    }
                }
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

    /// Consume a whole interpolated string (`"a#{expr}b"`) into `tokens` as
    /// its raw pieces. The tokenizer must be driven part-by-part — after each
    /// `#{...}` it resumes the string with `scan_string_template` — but every
    /// piece is still a plain token, so raw token sequences can hold it.
    pub(super) fn consume_str_template_tokens_into(
        &mut self,
        tokens: &mut oxc_allocator::Vec<'a, TokenWithSpan<'a>>,
    ) -> PResult<()> {
        let head = bump!(self);
        let quote = head.span.start;
        let quote = self.source[quote..].chars().next().unwrap_or('"');
        let mut tail = matches!(&head.token, Token::StrTemplate(template) if template.tail);
        tokens.push(head);
        while !tail {
            // Each non-tail part ends at `#{` with the `#` consumed, so the
            // interpolation opens with a bare `{` (like `SassInterpolatedStr`).
            let lbrace = bump!(self);
            debug_assert!(matches!(lbrace.token, Token::LBrace(..)));
            tokens.push(lbrace);
            // the expression's tokens, balanced to the interpolation's `}`
            let mut depth = 0u32;
            loop {
                match &peek!(self).token {
                    Token::Eof(..) => return Ok(()),
                    Token::StrTemplate(..) => {
                        self.consume_str_template_tokens_into(tokens)?;
                        continue;
                    }
                    Token::LBrace(..) | Token::HashLBrace(..) => depth += 1,
                    Token::RBrace(..) => {
                        if depth == 0 {
                            tokens.push(bump!(self));
                            break;
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
                tokens.push(bump!(self));
            }
            let (template, span) = self.tokenizer.scan_string_template(quote)?;
            tail = template.tail;
            tokens.push(TokenWithSpan { token: Token::StrTemplate(template), span });
        }
        Ok(())
    }
}

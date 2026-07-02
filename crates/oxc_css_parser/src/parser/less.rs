use super::{
    Parser,
    state::{LESS_CTX_ALLOW_DIV, LESS_CTX_ALLOW_KEYFRAME_BLOCK, ParserState, QualifiedRuleContext},
};
use crate::{
    Parse, arena_box, arena_vec,
    ast::*,
    bump,
    config::Syntax,
    eat,
    error::{Error, ErrorKind, PResult},
    expect, expect_without_ws_or_comments, peek,
    pos::{Span, Spanned},
    tokenizer::{Token, TokenWithSpan},
    util,
};
use std::mem;

const PRECEDENCE_AND: u8 = 2;
const PRECEDENCE_OR: u8 = 1;

const PRECEDENCE_MULTIPLY: u8 = 2;
const PRECEDENCE_PLUS: u8 = 1;

impl<'a> Parser<'a> {
    pub(super) fn parse_less_condition(
        &mut self,
        needs_parens: bool,
    ) -> PResult<LessCondition<'a>> {
        self.parse_less_condition_recursively(needs_parens, 0)
    }

    fn parse_less_condition_atom(&mut self) -> PResult<LessCondition<'a>> {
        let left =
            self.parse_less_operation(/* allow_mixin_call */ false).map(LessCondition::Value)?;

        let op = match &peek!(self).token {
            Token::GreaterThan(..) => LessBinaryConditionOperator {
                kind: LessBinaryConditionOperatorKind::GreaterThan,
                span: bump!(self).span,
            },
            Token::GreaterThanEqual(..) => LessBinaryConditionOperator {
                kind: LessBinaryConditionOperatorKind::GreaterThanOrEqual,
                span: bump!(self).span,
            },
            Token::LessThan(..) => LessBinaryConditionOperator {
                kind: LessBinaryConditionOperatorKind::LessThan,
                span: bump!(self).span,
            },
            Token::LessThanEqual(..) => LessBinaryConditionOperator {
                kind: LessBinaryConditionOperatorKind::LessThanOrEqual,
                span: bump!(self).span,
            },
            Token::Equal(..) => {
                let eq_span = bump!(self).span;
                match peek!(self) {
                    TokenWithSpan { token: Token::GreaterThan(..), span: gt_span }
                        if eq_span.end == gt_span.start =>
                    {
                        LessBinaryConditionOperator {
                            kind: LessBinaryConditionOperatorKind::EqualOrGreaterThan,
                            span: Span { start: eq_span.start, end: bump!(self).span.end },
                        }
                    }
                    TokenWithSpan { token: Token::LessThan(..), span: lt_span }
                        if eq_span.end == lt_span.start =>
                    {
                        LessBinaryConditionOperator {
                            kind: LessBinaryConditionOperatorKind::EqualOrLessThan,
                            span: Span { start: eq_span.start, end: bump!(self).span.end },
                        }
                    }
                    _ => LessBinaryConditionOperator {
                        kind: LessBinaryConditionOperatorKind::Equal,
                        span: eq_span,
                    },
                }
            }
            _ => return Ok(left),
        };

        let right =
            self.parse_less_operation(/* allow_mixin_call */ false).map(LessCondition::Value)?;

        let span = Span { start: left.span().start, end: right.span().end };
        Ok(LessCondition::Binary(LessBinaryCondition {
            left: arena_box!(self, left),
            op,
            right: arena_box!(self, right),
            span,
        }))
    }

    /// The parenthesized part of a guard condition, after its `(` was
    /// consumed: guards are math mode (`when (8+(5-1) < 13)`). Returns the
    /// condition and the `)`'s end offset.
    fn parse_less_guard_paren_condition(
        &mut self,
        needs_parens: bool,
    ) -> PResult<(LessCondition<'a>, usize)> {
        let condition = self
            .with_state(ParserState {
                less_ctx: self.state.less_ctx | LESS_CTX_ALLOW_DIV,
                ..self.state.clone()
            })
            .parse_less_condition_inside_parens(needs_parens)?;
        let (_, Span { end, .. }) = expect!(self, RParen);
        Ok((condition, end))
    }

    fn parse_less_condition_inside_parens(
        &mut self,
        needs_parens: bool,
    ) -> PResult<LessCondition<'a>> {
        self.try_parse(|parser| {
            let condition = parser.parse_less_condition(needs_parens);
            match &condition {
                Ok(LessCondition::Parenthesized(LessParenthesizedCondition {
                    condition: inner_condition,
                    span,
                })) => match &**inner_condition {
                    LessCondition::Value(ComponentValue::LessBinaryOperation(..))
                        if matches!(
                            peek!(parser).token,
                            Token::GreaterThan(..)
                                | Token::GreaterThanEqual(..)
                                | Token::LessThan(..)
                                | Token::LessThanEqual(..)
                                | Token::Equal(..)
                                | Token::Plus(..)
                                | Token::Minus(..)
                                | Token::Asterisk(..)
                                | Token::Solidus(..)
                        ) =>
                    {
                        // special case:
                        // `when ((8 + 6) > 13)`
                        // the `(8 + 6)` above is operation, not condition
                        Err(Error { kind: ErrorKind::TryParseError, span: span.clone() })
                    }
                    _ => condition,
                },
                _ => condition,
            }
        })
        .or_else(|_| self.parse_less_condition_atom())
    }

    fn parse_less_condition_recursively(
        &mut self,
        needs_parens: bool,
        precedence: u8,
    ) -> PResult<LessCondition<'a>> {
        let mut left = if precedence >= PRECEDENCE_AND {
            match &peek!(self).token {
                Token::LParen(..) => {
                    let Span { start, .. } = bump!(self).span;
                    let (condition, end) = self.parse_less_guard_paren_condition(needs_parens)?;
                    LessCondition::Parenthesized(LessParenthesizedCondition {
                        condition: arena_box!(self, condition),
                        span: Span { start, end },
                    })
                }
                Token::Ident(ident) if ident.raw == "not" => {
                    let Span { start, .. } = bump!(self).span;
                    let (condition, end) = if eat!(self, LParen).is_some() {
                        self.parse_less_guard_paren_condition(needs_parens)?
                    } else {
                        // less.js also accepts a bare operand: `when not @a`
                        let condition = self.parse_less_condition_atom()?;
                        let end = condition.span().end;
                        (condition, end)
                    };
                    LessCondition::Negated(LessNegatedCondition {
                        condition: arena_box!(self, condition),
                        span: Span { start, end },
                    })
                }
                _ => {
                    if needs_parens {
                        use crate::{token::LParen, tokenizer::TokenSymbol};
                        let TokenWithSpan { token, span } = bump!(self);
                        return Err(Error {
                            kind: ErrorKind::Unexpected(LParen::symbol(), token.symbol()),
                            span,
                        });
                    } else {
                        self.parse_less_condition_atom()?
                    }
                }
            }
        } else {
            self.parse_less_condition_recursively(needs_parens, precedence + 1)?
        };

        loop {
            let op = match &peek!(self).token {
                Token::Ident(token) if token.raw == "and" && precedence == PRECEDENCE_AND => {
                    LessBinaryConditionOperator {
                        kind: LessBinaryConditionOperatorKind::And,
                        span: bump!(self).span,
                    }
                }
                Token::Ident(token) if token.raw == "or" && precedence == PRECEDENCE_OR => {
                    LessBinaryConditionOperator {
                        kind: LessBinaryConditionOperatorKind::Or,
                        span: bump!(self).span,
                    }
                }
                _ => break,
            };

            // multiple conditions in Less are right-associated
            let right = self.parse_less_condition_recursively(needs_parens, precedence)?;

            let span = Span { start: left.span().start, end: right.span().end };
            left = LessCondition::Binary(LessBinaryCondition {
                left: arena_box!(self, left),
                op,
                right: arena_box!(self, right),
                span,
            });
        }

        Ok(left)
    }

    pub(super) fn parse_less_interpolated_ident(&mut self) -> PResult<InterpolableIdent<'a>> {
        debug_assert_eq!(self.syntax, Syntax::Less);

        let (first, Span { start, mut end }) = match peek!(self) {
            TokenWithSpan { token: Token::Ident(..), .. } => {
                let (ident, ident_span) = expect!(self, Ident);
                (
                    LessInterpolatedIdentElement::Static(
                        self.interpolable_ident_static_part(ident, ident_span.clone()),
                    ),
                    ident_span,
                )
            }
            TokenWithSpan { token: Token::AtLBraceVar(..), .. } => {
                let interpolation = self.parse::<LessVariableInterpolation>()?;
                let span = interpolation.span.clone();
                (LessInterpolatedIdentElement::Variable(interpolation), span)
            }
            TokenWithSpan { token: Token::DollarLBraceVar(..), .. }
                if matches!(
                    self.state.qualified_rule_ctx,
                    Some(QualifiedRuleContext::DeclarationName)
                ) =>
            {
                let interpolation = self.parse::<LessPropertyInterpolation>()?;
                let span = interpolation.span.clone();
                (LessInterpolatedIdentElement::Property(interpolation), span)
            }
            TokenWithSpan { token, span } => {
                use crate::{
                    token::{AtLBraceVar, Ident},
                    tokenizer::TokenSymbol,
                };
                return Err(Error {
                    kind: ErrorKind::ExpectOneOf(
                        vec![Ident::symbol(), AtLBraceVar::symbol()],
                        token.symbol(),
                    ),
                    span: span.clone(),
                });
            }
        };

        let mut elements = self.parse_less_interpolated_ident_rest(&mut end)?;
        if elements.is_empty()
            && let LessInterpolatedIdentElement::Static(ident) = first
        {
            return Ok(InterpolableIdent::Literal(Ident {
                name: ident.value,
                raw: ident.raw,
                span: ident.span,
            }));
        }

        elements.insert(0, first);
        Ok(InterpolableIdent::LessInterpolated(LessInterpolatedIdent {
            elements,
            span: Span { start, end },
        }))
    }

    pub(super) fn parse_less_interpolated_ident_rest(
        &mut self,
        end: &mut usize,
    ) -> PResult<oxc_allocator::Vec<'a, LessInterpolatedIdentElement<'a>>> {
        let mut elements = arena_vec!(self);
        loop {
            if let Some((token, span)) = self.tokenizer.scan_ident_template()? {
                *end = span.end;
                elements.push(LessInterpolatedIdentElement::Static(
                    self.interpolable_ident_static_part(token, span),
                ));
            } else {
                match peek!(self) {
                    TokenWithSpan { token: Token::AtLBraceVar(..), span: at_lbrace_var_span }
                        if *end == at_lbrace_var_span.start =>
                    {
                        let variable = self.parse::<LessVariableInterpolation>()?;
                        *end = variable.span.end;
                        elements.push(LessInterpolatedIdentElement::Variable(variable));
                    }
                    TokenWithSpan {
                        token: Token::DollarLBraceVar(..),
                        span: dollar_lbrace_var_span,
                    } if matches!(
                        self.state.qualified_rule_ctx,
                        Some(QualifiedRuleContext::DeclarationName)
                    ) && *end == dollar_lbrace_var_span.start =>
                    {
                        let property = self.parse::<LessPropertyInterpolation>()?;
                        *end = property.span.end;
                        elements.push(LessInterpolatedIdentElement::Property(property));
                    }
                    _ => return Ok(elements),
                }
            }
        }
    }

    pub(super) fn parse_less_maybe_mixin_call_or_with_lookups(
        &mut self,
    ) -> PResult<ComponentValue<'a>> {
        let mixin_call = self.parse::<LessMixinCall>()?;
        if matches!(peek!(self).token, Token::LBracket(..)) {
            let lookups = self.parse::<LessLookups>()?;
            let span = Span { start: mixin_call.span.start, end: lookups.span.end };
            Ok(ComponentValue::LessNamespaceValue(arena_box!(
                self,
                LessNamespaceValue {
                    callee: LessNamespaceValueCallee::LessMixinCall(mixin_call),
                    lookups,
                    span,
                }
            )))
        } else {
            Ok(ComponentValue::LessMixinCall(arena_box!(self, mixin_call)))
        }
    }

    pub(super) fn parse_less_maybe_variable_or_with_lookups(
        &mut self,
    ) -> PResult<ComponentValue<'a>> {
        let variable = self.parse::<LessVariable>()?;
        match peek!(self) {
            // a detached-ruleset call in value position: `a: @a();`
            TokenWithSpan { token: Token::LParen(..), span } if variable.span.end == span.start => {
                bump!(self);
                let (_, Span { end, .. }) = expect!(self, RParen);
                let span = Span { start: variable.span.start, end };
                return Ok(ComponentValue::LessVariableCall(LessVariableCall { variable, span }));
            }
            TokenWithSpan { token: Token::LBracket(..), span }
                if variable.span.end == span.start =>
            {
                let lookups = self.parse::<LessLookups>()?;
                let span = Span { start: variable.span.start, end: lookups.span.end };
                Ok(ComponentValue::LessNamespaceValue(arena_box!(
                    self,
                    LessNamespaceValue {
                        callee: LessNamespaceValueCallee::LessVariable(variable),
                        lookups,
                        span,
                    }
                )))
            }
            _ => Ok(ComponentValue::LessVariable(variable)),
        }
    }

    pub(super) fn parse_less_operation(
        &mut self,
        allow_mixin_call: bool,
    ) -> PResult<ComponentValue<'a>> {
        self.parse_less_operation_recursively(allow_mixin_call, 0)
    }

    fn parse_less_operation_recursively(
        &mut self,
        allow_mixin_call: bool,
        precedence: u8,
    ) -> PResult<ComponentValue<'a>> {
        let mut left = if precedence >= PRECEDENCE_MULTIPLY {
            match peek!(self).token {
                Token::LParen(..) => self
                    .parse_less_parenthesized_operation(allow_mixin_call)
                    .map(ComponentValue::LessParenthesizedOperation)?,
                Token::Minus(..) => {
                    // less.js's keyword regex also matches a bare `-`
                    // (`a:hover when (2 = true) {5:-}`); only treat it as one
                    // when a terminator follows immediately
                    let end = peek!(self).span.end;
                    if matches!(self.source.as_bytes().get(end), None | Some(b';' | b'}')) {
                        let span = bump!(self).span;
                        ComponentValue::InterpolableIdent(InterpolableIdent::Literal(Ident {
                            name: "-",
                            raw: "-",
                            span,
                        }))
                    } else {
                        self.parse::<LessNegativeValue>().map(ComponentValue::LessNegativeValue)?
                    }
                }
                _ => {
                    let value = self.parse_component_value_atom()?;
                    if let ComponentValue::LessMixinCall(mixin_call) = &value
                        && !allow_mixin_call
                    {
                        self.recoverable_errors.push(Error {
                            kind: ErrorKind::UnexpectedLessMixinCall,
                            span: mixin_call.span.clone(),
                        });
                    }
                    value
                }
            }
        } else {
            self.parse_less_operation_recursively(allow_mixin_call, precedence + 1)?
        };

        loop {
            let op = match peek!(self) {
                TokenWithSpan { token: Token::Asterisk(..), .. }
                    if precedence == PRECEDENCE_MULTIPLY =>
                {
                    LessOperationOperator {
                        kind: LessOperationOperatorKind::Multiply,
                        span: bump!(self).span,
                    }
                }
                TokenWithSpan { token: Token::Solidus(..), .. }
                    if precedence == PRECEDENCE_MULTIPLY
                        && (self.state.less_ctx & LESS_CTX_ALLOW_DIV != 0
                            || can_be_division_operand(&left)) =>
                {
                    LessOperationOperator {
                        kind: LessOperationOperatorKind::Division,
                        span: bump!(self).span,
                    }
                }
                TokenWithSpan { token: Token::Dot(..), .. }
                    if precedence == PRECEDENCE_MULTIPLY =>
                {
                    // `./` is also division
                    let Span { start, .. } = bump!(self).span;
                    let (_, Span { end, .. }) = expect_without_ws_or_comments!(self, Solidus);
                    LessOperationOperator {
                        kind: LessOperationOperatorKind::Division,
                        span: Span { start, end },
                    }
                }
                // A `+`/`-` is a binary operator only when followed by
                // whitespace. lessc is whitespace-sensitive here: `@a - @b`
                // is subtraction, but `@a -@b` is two values (`-@b` is a
                // signed value, not an operator) — see the `margin` shorthand.
                TokenWithSpan { token: Token::Plus(..), span }
                    if precedence == PRECEDENCE_PLUS
                        && (is_followed_by_whitespace(self.source, span.end)
                            || self.state.less_ctx & LESS_CTX_ALLOW_DIV != 0) =>
                {
                    LessOperationOperator {
                        kind: LessOperationOperatorKind::Plus,
                        span: bump!(self).span,
                    }
                }
                TokenWithSpan { token: Token::Minus(..), span }
                    if precedence == PRECEDENCE_PLUS
                        && (is_followed_by_whitespace(self.source, span.end)
                            || self.state.less_ctx & LESS_CTX_ALLOW_DIV != 0) =>
                {
                    LessOperationOperator {
                        kind: LessOperationOperatorKind::Minus,
                        span: bump!(self).span,
                    }
                }
                TokenWithSpan { token: Token::Number(token), span }
                    if precedence == PRECEDENCE_PLUS
                        && (token.raw.starts_with('+')
                            || token.raw.starts_with('-') && span.start == left.span().end) =>
                {
                    let (number, number_span) = expect!(self, Number);
                    let op = LessOperationOperator {
                        kind: if number.raw.starts_with('+') {
                            LessOperationOperatorKind::Plus
                        } else {
                            LessOperationOperatorKind::Minus
                        },
                        span: Span { start: number_span.start, end: number_span.start + 1 },
                    };
                    let span = Span { start: left.span().start, end: number_span.end };
                    let right = {
                        let span = Span { start: number_span.start + 1, end: number_span.end };
                        let raw = unsafe { number.raw.get_unchecked(1..number.raw.len()) };
                        raw.parse()
                            .map_err(|_| Error {
                                kind: ErrorKind::InvalidNumber,
                                span: span.clone(),
                            })
                            .map(|value| ComponentValue::Number(Number { value, raw, span }))?
                    };
                    left = ComponentValue::LessBinaryOperation(LessBinaryOperation {
                        left: arena_box!(self, left),
                        op,
                        right: arena_box!(self, right),
                        span,
                    });
                    continue;
                }
                TokenWithSpan { token: Token::Dimension(token), span }
                    if precedence == PRECEDENCE_PLUS
                        && (token.value.raw.starts_with('+')
                            || token.value.raw.starts_with('-')
                                && span.start == left.span().end) =>
                {
                    let (dimension, dimension_span) = expect!(self, Dimension);
                    let op = LessOperationOperator {
                        kind: if dimension.value.raw.starts_with('+') {
                            LessOperationOperatorKind::Plus
                        } else {
                            LessOperationOperatorKind::Minus
                        },
                        span: Span { start: dimension_span.start, end: dimension_span.start + 1 },
                    };
                    let mut right = {
                        self.dimension(
                            crate::token::Dimension {
                                value: crate::token::Number {
                                    raw: unsafe {
                                        dimension
                                            .value
                                            .raw
                                            .get_unchecked(1..dimension.value.raw.len())
                                    },
                                },
                                unit: dimension.unit,
                            },
                            Span { start: dimension_span.start + 1, end: dimension_span.end },
                        )
                        .map(ComponentValue::Dimension)?
                    };
                    // multiplication binds tighter than the split-off sign:
                    // `(6px-1px*2)` is `6px - (1px * 2)`
                    while matches!(&peek!(self).token, Token::Asterisk(..) | Token::Solidus(..)) {
                        let mul_op = LessOperationOperator {
                            kind: if matches!(&peek!(self).token, Token::Asterisk(..)) {
                                LessOperationOperatorKind::Multiply
                            } else {
                                LessOperationOperatorKind::Division
                            },
                            span: bump!(self).span,
                        };
                        let mul_rhs = self.parse_less_operation_recursively(
                            allow_mixin_call,
                            PRECEDENCE_MULTIPLY + 1,
                        )?;
                        let span = Span { start: right.span().start, end: mul_rhs.span().end };
                        right = ComponentValue::LessBinaryOperation(LessBinaryOperation {
                            left: arena_box!(self, right),
                            op: mul_op,
                            right: arena_box!(self, mul_rhs),
                            span,
                        });
                    }
                    let span = Span { start: left.span().start, end: right.span().end };
                    left = ComponentValue::LessBinaryOperation(LessBinaryOperation {
                        left: arena_box!(self, left),
                        op,
                        right: arena_box!(self, right),
                        span,
                    });
                    continue;
                }
                _ => break,
            };

            let right = self.parse_less_operation_recursively(allow_mixin_call, precedence + 1)?;
            let span = Span { start: left.span().start, end: right.span().end };
            left = ComponentValue::LessBinaryOperation(LessBinaryOperation {
                left: arena_box!(self, left),
                op,
                right: arena_box!(self, right),
                span,
            });
        }

        Ok(left)
    }

    fn parse_less_parenthesized_operation(
        &mut self,
        allow_mixin_call: bool,
    ) -> PResult<LessParenthesizedOperation<'a>> {
        let (_, Span { start, .. }) = expect!(self, LParen);
        let operation = self
            .with_state(ParserState {
                less_ctx: self.state.less_ctx | LESS_CTX_ALLOW_DIV,
                ..self.state.clone()
            })
            .parse_less_operation(allow_mixin_call)?;
        let (_, Span { end, .. }) = expect!(self, RParen);
        Ok(LessParenthesizedOperation {
            operation: arena_box!(self, operation),
            span: Span { start, end },
        })
    }

    pub(super) fn parse_less_qualified_rule(&mut self) -> PResult<Statement<'a>> {
        debug_assert_eq!(self.syntax, Syntax::Less);

        let selector_list = self
            .with_state(ParserState {
                qualified_rule_ctx: Some(QualifiedRuleContext::Selector),
                ..self.state
            })
            .parse::<SelectorList>()?;

        match &peek!(self).token {
            Token::Ident(ident) if ident.raw == "when" => {
                // less.js: "Guards are only currently allowed on a single
                // selector." — a guard on a selector list is rejected whether
                // the comma comes before or after the `when`.
                if selector_list.selectors.len() > 1 {
                    let span = peek!(self).span.clone();
                    return Err(Error {
                        kind: ErrorKind::LessGuardOnMultipleComplexSelectors,
                        span,
                    });
                }
                let guard = self.parse::<LessConditions>()?;
                let block = self.parse::<SimpleBlock>()?;
                let span = Span { start: selector_list.span.start, end: block.span.end };
                return Ok(Statement::LessConditionalQualifiedRule(LessConditionalQualifiedRule {
                    selector: selector_list,
                    guard,
                    block,
                    span,
                }));
            }
            _ => {}
        }

        let block = self.parse::<SimpleBlock>()?;
        let span = Span { start: selector_list.span.start, end: block.span.end };
        Ok(Statement::QualifiedRule(QualifiedRule { selector: selector_list, block, span }))
    }

    pub(super) fn parse_maybe_hex_color_or_less_mixin_call(
        &mut self,
    ) -> PResult<ComponentValue<'a>> {
        debug_assert_eq!(self.syntax, Syntax::Less);

        let attempt = self.try_parse(|parser| {
            let hex_color = parser.parse::<HexColor>()?;
            match peek!(parser) {
                TokenWithSpan { token: Token::LParen(..), span } => {
                    Err(Error { kind: ErrorKind::TryParseError, span: span.clone() })
                }
                TokenWithSpan {
                    token: Token::LBracket(..) | Token::Dot(..) | Token::Hash(..),
                    span,
                } if hex_color.span.end == span.start => {
                    Err(Error { kind: ErrorKind::TryParseError, span: span.clone() })
                }
                _ => Ok(hex_color),
            }
        });
        match attempt {
            Err(Error { kind: ErrorKind::TryParseError, .. }) => {
                self.parse_less_maybe_mixin_call_or_with_lookups()
            }
            hex_color => hex_color.map(ComponentValue::HexColor),
        }
    }

    pub(super) fn parse_maybe_less_list(
        &mut self,
        allow_comma: bool,
    ) -> PResult<ComponentValue<'a>> {
        use util::ListSeparatorKind;

        let single_value = if allow_comma {
            self.parse_maybe_less_list(false)?
        } else if let Token::Exclamation(..) = peek!(self).token {
            self.parse().map(ComponentValue::ImportantAnnotation)?
        } else {
            self.parse_less_operation(/* allow_mixin_call */ true)?
        };

        let mut elements = arena_vec!(self);
        let mut comma_spans: Option<oxc_allocator::Vec<'a, Span>> = None;
        let mut separator = ListSeparatorKind::Unknown;
        let mut end = single_value.span().end;
        loop {
            match peek!(self).token {
                Token::LBrace(..)
                | Token::RBrace(..)
                | Token::RParen(..)
                | Token::Semicolon(..)
                | Token::Colon(..)
                | Token::DotDotDot(..)
                | Token::Eof(..) => break,
                Token::Comma(..) => {
                    if !allow_comma {
                        break;
                    }
                    if separator == ListSeparatorKind::Space {
                        break;
                    } else {
                        if separator == ListSeparatorKind::Unknown {
                            separator = ListSeparatorKind::Comma;
                        }
                        let TokenWithSpan { span, .. } = bump!(self);
                        end = span.end;
                        if let Some(spans) = &mut comma_spans {
                            spans.push(span);
                        } else {
                            comma_spans = Some(arena_vec!(self; span));
                        }
                    }
                }
                Token::Exclamation(..) => {
                    if let Ok(important_annotation) = self.try_parse(ImportantAnnotation::parse) {
                        if end < important_annotation.span.start
                            && separator == ListSeparatorKind::Unknown
                        {
                            separator = ListSeparatorKind::Space;
                        }
                        end = important_annotation.span.end;
                        elements.push(ComponentValue::ImportantAnnotation(important_annotation));
                    } else {
                        break;
                    }
                }
                _ => {
                    if separator == ListSeparatorKind::Unknown {
                        separator = ListSeparatorKind::Space;
                    }
                    let item = if separator == ListSeparatorKind::Comma {
                        self.parse_maybe_less_list(false)?
                    } else {
                        self.parse_less_operation(/* allow_mixin_call */ true)?
                    };
                    end = item.span().end;
                    elements.push(item);
                }
            }
        }

        if elements.is_empty() && separator != ListSeparatorKind::Comma {
            // If there is a trailing comma it can be a list,
            // though there is only one element.
            Ok(single_value)
        } else {
            debug_assert_ne!(separator, ListSeparatorKind::Unknown);

            let span = Span { start: single_value.span().start, end };
            elements.insert(0, single_value);
            Ok(ComponentValue::LessList(LessList { elements, comma_spans, span }))
        }
    }
}

impl<'a> Parse<'a> for LessConditions<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let when_span = match bump!(input) {
            TokenWithSpan { token: Token::Ident(ident), span } if ident.raw == "when" => span,
            TokenWithSpan { span, .. } => {
                return Err(Error { kind: ErrorKind::ExpectLessKeyword("when"), span });
            }
        };

        let first = input.parse_less_condition(true)?;
        let mut span = first.span().clone();

        let mut conditions = arena_vec!(input; first);
        let mut comma_spans = arena_vec!(input);
        // A comma may also separate the next guarded selector of the rule
        // (`.a when (..), .b when (..) {`), so only consume it when a
        // condition actually follows.
        while matches!(peek!(input).token, Token::Comma(..)) {
            let Ok((comma_span, condition)) = input.try_parse(|p| {
                let (_, comma_span) = expect!(p, Comma);
                let condition = p.parse_less_condition(true)?;
                Ok((comma_span, condition))
            }) else {
                break;
            };
            comma_spans.push(comma_span);
            conditions.push(condition);
        }
        debug_assert_eq!(comma_spans.len() + 1, conditions.len());

        if let Some(last) = conditions.last() {
            span.end = last.span().end;
        }
        Ok(LessConditions { conditions, when_span, comma_spans, span })
    }
}

impl<'a> Parse<'a> for LessDetachedRuleset<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let block = input.parse::<SimpleBlock>()?;
        let span = block.span.clone();
        Ok(LessDetachedRuleset { block, span })
    }
}

impl<'a> Parse<'a> for LessEscapedStr<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, Span { start, .. }) = expect!(input, Tilde);
        let str = {
            let (str, span) = input.tokenizer.scan_string_only()?;
            input.str(str, span)
        };
        let span = Span { start, end: str.span().end };
        Ok(LessEscapedStr { str, span })
    }
}

impl<'a> Parse<'a> for LessExtend<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let mut selector = input.parse::<ComplexSelector>()?;

        let span = selector.span.clone();
        let mut all = None;

        if let [
            ..,
            complex_child,
            ComplexSelectorChild::Combinator(Combinator {
                kind: CombinatorKind::Descendant, ..
            }),
            ComplexSelectorChild::CompoundSelector(CompoundSelector { children, .. }),
        ] = &selector.children[..]
            && let [
                SimpleSelector::Type(TypeSelector::TagName(TagNameSelector {
                    name:
                        WqName {
                            name: InterpolableIdent::Literal(token_all @ Ident { raw: "all", .. }),
                            prefix: None,
                            ..
                        },
                    ..
                })),
            ] = &children[..]
        {
            all = Some(Ident {
                name: token_all.name,
                raw: token_all.raw,
                span: token_all.span.clone(),
            });
            selector.span.end = complex_child.span().end;
            let len = selector.children.len();
            selector.children.truncate(len - 2);
        }

        Ok(LessExtend { selector, all, span })
    }
}

impl<'a> Parse<'a> for LessExtendList<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert_eq!(input.syntax, Syntax::Less);

        let first = input.parse::<LessExtend>()?;
        let mut span = first.span.clone();

        let mut elements = arena_vec!(input; first);
        let mut comma_spans = arena_vec!(input);
        while let Some((_, comma_span)) = eat!(input, Comma) {
            comma_spans.push(comma_span);
            elements.push(input.parse()?);
        }
        debug_assert_eq!(comma_spans.len() + 1, elements.len());

        if let Some(last) = elements.last() {
            span.end = last.span.end;
        }
        Ok(LessExtendList { elements, comma_spans, span })
    }
}

impl<'a> Parse<'a> for LessExtendRule<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let nesting_selector = input.parse::<NestingSelector>()?;
        if nesting_selector.suffix.is_some() {
            return Err(Error {
                kind: ErrorKind::ExpectLessExtendRule,
                span: nesting_selector.span,
            });
        }

        let pseudo_class_selector = input.parse::<PseudoClassSelector>()?;
        util::assert_no_ws_or_comment(&nesting_selector.span, &pseudo_class_selector.span)?;
        let span = Span { start: nesting_selector.span.start, end: pseudo_class_selector.span.end };

        let InterpolableIdent::Literal(name_of_extend @ Ident { raw: "extend", .. }) =
            pseudo_class_selector.name
        else {
            return Err(Error { kind: ErrorKind::ExpectLessExtendRule, span });
        };
        let Some(PseudoClassSelectorArg {
            kind: PseudoClassSelectorArgKind::LessExtendList(extend),
            ..
        }) = pseudo_class_selector.arg
        else {
            return Err(Error { kind: ErrorKind::ExpectLessExtendRule, span });
        };

        Ok(LessExtendRule { nesting_selector, name_of_extend, extend, span })
    }
}

impl<'a> Parse<'a> for LessFormatFunction {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, span) = expect!(input, Percent);
        Ok(LessFormatFunction { span })
    }
}

impl<'a> Parse<'a> for LessImportOptions<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, Span { start, .. }) = expect!(input, LParen);

        let mut names = input.vec_with_capacity(1);
        let mut comma_spans = arena_vec!(input);
        while let Token::Ident(crate::token::Ident {
            raw: "less" | "css" | "multiple" | "once" | "inline" | "reference" | "optional",
            ..
        }) = peek!(input).token
        {
            names.push(input.parse()?);
            if !matches!(peek!(input).token, Token::RParen(..)) {
                comma_spans.push(expect!(input, Comma).1);
            }
        }
        debug_assert!(names.len() - comma_spans.len() <= 1);

        let (_, Span { end, .. }) = expect!(input, RParen);

        Ok(LessImportOptions { names, comma_spans, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for LessImportPrelude<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let options = input.parse::<LessImportOptions>()?;
        let start = options.span.start;

        let href = match &peek!(input).token {
            Token::Str(..) | Token::StrTemplate(..) => input.parse().map(ImportPreludeHref::Str)?,
            _ => input.parse().map(ImportPreludeHref::Url)?,
        };
        let mut end = href.span().end;

        let media = if matches!(peek!(input).token, Token::Semicolon(..)) {
            None
        } else {
            let media = input.parse::<MediaQueryList>()?;
            end = media.span.end;
            Some(media)
        };

        Ok(LessImportPrelude { href, options, media, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for LessInterpolatedStr<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (first, first_span) = expect!(input, StrTemplate);
        let quote = first.raw.chars().next().unwrap();
        debug_assert!(quote == '\'' || quote == '"');
        let mut span = first_span.clone();
        let mut elements = arena_vec!(
            input;
            LessInterpolatedStrElement::Static(
                input.interpolable_str_static_part(first, first_span)
            )
        );

        let mut is_parsing_static_part = false;
        loop {
            if is_parsing_static_part {
                let (token, str_tpl_span) = input.tokenizer.scan_string_template(quote)?;
                let tail = token.tail;
                let end = str_tpl_span.end;
                elements.push(LessInterpolatedStrElement::Static(
                    input.interpolable_str_static_part(token, str_tpl_span),
                ));
                if tail {
                    span.end = end;
                    break;
                }
            } else {
                // '@' or '$' is consumed, so '{' left only
                let start = expect!(input, LBrace).1.start - 1;
                // Less interpolation names may start with a digit (`@{3}`).
                let (name, name_span) = expect_without_ws_or_comments!(
                    input, Ident, /* allow_leading_digit */ true
                );

                let end = expect!(input, RBrace).1.end;
                elements.push(match input.source.as_bytes().get(start) {
                    Some(b'@') => LessInterpolatedStrElement::Variable(LessVariableInterpolation {
                        name: input.ident(name, name_span),
                        span: Span { start, end },
                    }),
                    Some(b'$') => LessInterpolatedStrElement::Property(LessPropertyInterpolation {
                        name: input.ident(name, name_span),
                        span: Span { start, end },
                    }),
                    _ => unreachable!(),
                });
            }
            is_parsing_static_part = !is_parsing_static_part;
        }

        Ok(LessInterpolatedStr { elements, span })
    }
}

impl<'a> Parse<'a> for LessJavaScriptSnippet<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let tilde = eat!(input, Tilde);
        let (token, span) = expect!(input, BacktickCode);

        Ok(LessJavaScriptSnippet {
            code: &token.raw[1..token.raw.len() - 1],
            raw: token.raw,
            escaped: tilde.is_some(),
            span: Span {
                start: tilde.map(|(_, span)| span.start).unwrap_or(span.start),
                end: span.end,
            },
        })
    }
}

impl<'a> Parse<'a> for LessListFunction {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, span) = expect!(input, Tilde);
        Ok(LessListFunction { span })
    }
}

impl<'a> Parse<'a> for LessLookup<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert_eq!(input.syntax, Syntax::Less);

        let (_, Span { start, .. }) = expect!(input, LBracket);
        let name =
            if let Token::RBracket(..) = peek!(input).token { None } else { Some(input.parse()?) };
        let (_, Span { end, .. }) = expect!(input, RBracket);
        Ok(LessLookup { name, span: Span { start, end } })
    }
}

impl<'a> Parse<'a> for LessLookupName<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert_eq!(input.syntax, Syntax::Less);

        let is_dollar = {
            let TokenWithSpan { token, span } = peek!(input);
            matches!(token, Token::Unknown(..))
                && input.source.as_bytes().get(span.start) == Some(&b'$')
        };
        match peek!(input).token {
            Token::AtKeyword(..) => input.parse().map(LessLookupName::LessVariable),
            Token::At(..) => input.parse().map(LessLookupName::LessVariableVariable),
            Token::DollarVar(..) => input.parse().map(LessLookupName::LessPropertyVariable),
            // `[$@var]` — a property lookup whose name is interpolated from a
            // variable; the lone `$` arrives as an <unknown-token>
            Token::Unknown(..) if is_dollar => {
                let dollar_span = bump!(input).span;
                let variable = input.parse::<LessVariable>()?;
                util::assert_no_ws_or_comment(&dollar_span, &variable.span)?;
                let span = Span { start: dollar_span.start, end: variable.span.end };
                Ok(LessLookupName::LessPropertyInterpolation(LessPropertyInterpolation {
                    name: variable.name,
                    span,
                }))
            }
            _ => input.parse().map(LessLookupName::Ident),
        }
    }
}

impl<'a> Parse<'a> for LessLookups<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert_eq!(input.syntax, Syntax::Less);

        let first = input.parse::<LessLookup>()?;
        let mut span = first.span.clone();

        let mut lookups = arena_vec!(input; first);
        while let Token::LBracket(..) = peek!(input).token {
            lookups.push(input.parse()?);
        }

        if let Some(last) = lookups.last() {
            span.end = last.span.end;
        }
        Ok(LessLookups { lookups, span })
    }
}

impl<'a> Parse<'a> for LessMixinCall<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert_eq!(input.syntax, Syntax::Less);

        let callee = input.parse::<LessMixinCallee>()?;

        let mut end = callee.span.end;
        let args = if let Some((_, lparen_span)) = eat!(input, LParen) {
            let mut semicolon_comes_at = 0;
            let mut args = arena_vec!(input);
            let mut comma_spans = arena_vec!(input);
            let mut semicolon_spans = arena_vec!(input);
            loop {
                match peek!(input).token {
                    Token::RParen(..) => {
                        let TokenWithSpan { span, .. } = bump!(input);
                        if semicolon_comes_at > 0 {
                            let comma_spans = mem::replace(&mut comma_spans, arena_vec!(input));
                            wrap_less_mixin_args_into_less_list(
                                input.allocator(),
                                &mut args,
                                comma_spans,
                                semicolon_comes_at,
                            )
                            .map_err(|kind| Error {
                                kind,
                                span: Span {
                                    // We've checked `semicolon_comes_at` must be greater than 0,
                                    // so `args` won't be empty.
                                    start: args.first().unwrap().span().start,
                                    end: args.last().unwrap().span().end,
                                },
                            })?;
                        }
                        end = span.end;
                        break;
                    }
                    Token::LBrace(..) => args.push(LessMixinArgument::Value(
                        ComponentValue::LessDetachedRuleset(input.parse()?),
                    )),
                    Token::Comma(..) => {
                        return Err(Error {
                            kind: ErrorKind::ExpectComponentValue,
                            span: bump!(input).span,
                        });
                    }
                    _ => 'maybe: {
                        let value = input.parse_maybe_less_list(/* allow_comma */ false)?;
                        let name = {
                            match value {
                                ComponentValue::LessVariable(variable) => {
                                    LessMixinParameterName::Variable(variable)
                                }
                                ComponentValue::LessPropertyVariable(property) => {
                                    LessMixinParameterName::PropertyVariable(property)
                                }
                                value => {
                                    args.push(LessMixinArgument::Value(value));
                                    break 'maybe;
                                }
                            }
                        };
                        if let Some((_, colon_span)) = eat!(input, Colon) {
                            let value = if matches!(peek!(input).token, Token::LBrace(..)) {
                                input.parse().map(ComponentValue::LessDetachedRuleset)?
                            } else {
                                input.parse_maybe_less_list(
                                    /* allow_comma */ semicolon_comes_at > 0,
                                )?
                            };
                            let span = Span { start: name.span().start, end: value.span().end };
                            args.push(LessMixinArgument::Named(LessMixinNamedArgument {
                                name,
                                colon_span,
                                value,
                                span,
                            }));
                        } else if let Some((_, dotdotdot_span)) = eat!(input, DotDotDot) {
                            // unlike definitions, call-site spreads may appear
                            // anywhere and repeat: `.m(@x..., @a: 0)`,
                            // `.aa(@y, @x..., and again, @y...)`
                            let span = Span { start: name.span().start, end: dotdotdot_span.end };
                            args.push(LessMixinArgument::Variadic(LessMixinVariadicArgument {
                                name,
                                span,
                            }));
                        } else {
                            args.push(LessMixinArgument::Value(match name {
                                LessMixinParameterName::Variable(variable) => {
                                    ComponentValue::LessVariable(variable)
                                }
                                LessMixinParameterName::PropertyVariable(property_variable) => {
                                    ComponentValue::LessPropertyVariable(property_variable)
                                }
                            }));
                        }
                    }
                };

                match peek!(input).token {
                    Token::RParen(..) => {}
                    Token::Comma(..) => {
                        comma_spans.push(bump!(input).span);
                    }
                    Token::Semicolon(..) => {
                        let TokenWithSpan { span, .. } = bump!(input);
                        let comma_spans = mem::replace(&mut comma_spans, arena_vec!(input));
                        wrap_less_mixin_args_into_less_list(
                            input.allocator(),
                            &mut args,
                            comma_spans,
                            semicolon_comes_at,
                        )
                        .map_err(|kind| Error { kind, span: span.clone() })?;
                        semicolon_comes_at = args.len();
                        semicolon_spans.push(span);
                    }
                    _ => {
                        let TokenWithSpan { token, span } = bump!(input);
                        use crate::{token::RParen, tokenizer::TokenSymbol};
                        return Err(Error {
                            kind: ErrorKind::Unexpected(RParen::symbol(), token.symbol()),
                            span,
                        });
                    }
                }
            }
            let is_comma_separated = semicolon_spans.is_empty();
            let separator_spans =
                if semicolon_spans.is_empty() { comma_spans } else { semicolon_spans };
            debug_assert!(args.len() - separator_spans.len() <= 1);
            Some(LessMixinArguments {
                args,
                is_comma_separated,
                separator_spans,
                span: Span { start: lparen_span.start, end },
            })
        } else {
            None
        };

        let important = if !matches!(
            input.state.qualified_rule_ctx,
            Some(QualifiedRuleContext::DeclarationValue)
        ) && matches!(peek!(input).token, Token::Exclamation(..))
        {
            input.parse::<ImportantAnnotation>().map(Some)?
        } else {
            None
        };

        let span = Span {
            start: callee.span.start,
            end: important.as_ref().map(|important| important.span.end).unwrap_or(end),
        };
        Ok(LessMixinCall { callee, args, important, span })
    }
}

impl<'a> Parser<'a> {
    fn parse_less_mixin_parameters(&mut self) -> PResult<LessMixinParameters<'a>> {
        let (_, lparen_span) = expect!(self, LParen);
        let rparen_span;
        let mut semicolon_comes_at = 0;
        let mut params = arena_vec!(self);
        let mut comma_spans = arena_vec!(self);
        let mut semicolon_spans = arena_vec!(self);
        'params: loop {
            match peek!(self).token {
                Token::RParen(..) => {
                    rparen_span = bump!(self).span;
                    break;
                }
                Token::DotDotDot(..) => {
                    let TokenWithSpan { span, .. } = bump!(self);
                    params.push(LessMixinParameter::Variadic(LessMixinVariadicParameter {
                        name: None,
                        span,
                    }));
                    eat!(self, Semicolon);
                    (_, rparen_span) = expect!(self, RParen);
                    break;
                }
                Token::Comma(..) => {
                    return Err(Error {
                        kind: ErrorKind::ExpectComponentValue,
                        span: bump!(self).span,
                    });
                }
                _ => 'maybe: {
                    let value = self
                        .with_state(ParserState {
                            less_ctx: self.state.less_ctx | LESS_CTX_ALLOW_DIV,
                            ..self.state.clone()
                        })
                        .parse::<ComponentValue>()?;
                    let name = {
                        match value {
                            ComponentValue::LessVariable(variable) => {
                                LessMixinParameterName::Variable(variable)
                            }
                            ComponentValue::LessPropertyVariable(property) => {
                                LessMixinParameterName::PropertyVariable(property)
                            }
                            value => {
                                let span = value.span().clone();
                                params.push(LessMixinParameter::Unnamed(
                                    LessMixinUnnamedParameter { value, span },
                                ));
                                break 'maybe;
                            }
                        }
                    };
                    let name_span = name.span();
                    if let Some((_, colon_span)) = eat!(self, Colon) {
                        let value = if matches!(peek!(self).token, Token::LBrace(..)) {
                            self.parse().map(ComponentValue::LessDetachedRuleset)?
                        } else {
                            self.with_state(ParserState {
                                less_ctx: self.state.less_ctx | LESS_CTX_ALLOW_DIV,
                                ..self.state.clone()
                            })
                            .parse_maybe_less_list(/* allow_comma */ false)?
                        };
                        let end = value.span().end;
                        let default_value = {
                            let span = Span { start: colon_span.start, end };
                            LessMixinNamedParameterDefaultValue { colon_span, value, span }
                        };
                        let span = Span { start: name_span.start, end };
                        params.push(LessMixinParameter::Named(LessMixinNamedParameter {
                            name,
                            value: Some(default_value),
                            span,
                        }));
                    } else if let Some((_, Span { end, .. })) = eat!(self, DotDotDot) {
                        let span = Span { start: name_span.start, end };
                        params.push(LessMixinParameter::Variadic(LessMixinVariadicParameter {
                            name: Some(name),
                            span,
                        }));
                        if let Some((_, semicolon_span)) = eat!(self, Semicolon) {
                            semicolon_spans.push(semicolon_span);
                        };
                        (_, rparen_span) = expect!(self, RParen);
                        break 'params;
                    } else {
                        let span = name_span.clone();
                        params.push(LessMixinParameter::Named(LessMixinNamedParameter {
                            name,
                            value: None,
                            span,
                        }));
                    }
                }
            }

            match &peek!(self).token {
                Token::RParen(..) => {
                    let span = bump!(self).span;
                    if semicolon_comes_at > 0 {
                        let comma_spans = mem::replace(&mut comma_spans, arena_vec!(self));
                        wrap_less_mixin_params_into_less_list(
                            self.allocator(),
                            &mut params,
                            comma_spans,
                            semicolon_comes_at,
                        )
                        .map_err(|kind| Error {
                            kind,
                            span: Span {
                                // We've checked `semicolon_comes_at` must be greater than 0,
                                // so `params` won't be empty.
                                start: params.first().unwrap().span().start,
                                end: params.last().unwrap().span().end,
                            },
                        })?;
                    }
                    rparen_span = span;
                    break;
                }
                Token::Comma(..) => {
                    comma_spans.push(bump!(self).span);
                }
                Token::Semicolon(..) => {
                    let span = bump!(self).span;
                    let comma_spans = mem::replace(&mut comma_spans, arena_vec!(self));
                    wrap_less_mixin_params_into_less_list(
                        self.allocator(),
                        &mut params,
                        comma_spans,
                        semicolon_comes_at,
                    )
                    .map_err(|kind| Error { kind, span: span.clone() })?;
                    semicolon_comes_at = params.len();
                    semicolon_spans.push(span);
                }
                // less.js also accepts space-separated parameters
                // (`.m(@a @b)`); each element becomes its own parameter
                _ => {}
            }
        }
        let is_comma_separated = semicolon_spans.is_empty();
        let separator_spans =
            if semicolon_spans.is_empty() { comma_spans } else { semicolon_spans };
        debug_assert!(separator_spans.len() <= params.len());
        Ok(LessMixinParameters {
            params,
            is_comma_separated,
            separator_spans,
            span: Span { start: lparen_span.start, end: rparen_span.end },
        })
    }

    /// `.(@v; @i) { ... }` / `#(@v, @k, @i) { ... }` — an anonymous mixin,
    /// used as a callback argument to `each()` and friends.
    pub(super) fn parse_less_anonymous_mixin(&mut self) -> PResult<LessAnonymousMixin<'a>> {
        debug_assert_eq!(self.syntax, Syntax::Less);

        let TokenWithSpan { token, span: head_span } = bump!(self);
        if !matches!(token, Token::Dot(..) | Token::NumberSign(..))
            || peek!(self).span.start != head_span.end
        {
            return Err(Error { kind: ErrorKind::TryParseError, span: head_span });
        }
        let params = self.parse_less_mixin_parameters()?;
        let block = self.parse::<SimpleBlock>()?;
        let span = Span { start: head_span.start, end: block.span.end };
        Ok(LessAnonymousMixin { params, block, span })
    }
}

impl<'a> Parse<'a> for LessMixinCallee<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let first_name = input.parse::<LessMixinName>()?;
        let mut span = first_name.span().clone();

        let mut children = arena_vec!(
            input;
            LessMixinCalleeChild { name: first_name, combinator: None, span: span.clone() }
        );
        loop {
            let combinator = eat!(input, GreaterThan)
                .map(|(_, span)| Combinator { kind: CombinatorKind::Child, span });
            let at_name = {
                let TokenWithSpan { token, span } = peek!(input);
                matches!(token, Token::Dot(..) | Token::Hash(..))
                    || (matches!(token, Token::Dimension(..))
                        && input.source.as_bytes().get(span.start) == Some(&b'.'))
            };
            if at_name {
                let name = input.parse::<LessMixinName>()?;
                let name_span = name.span();
                let span = Span {
                    start: combinator
                        .as_ref()
                        .map(|combinator| combinator.span.start)
                        .unwrap_or(name_span.start),
                    end: name_span.end,
                };
                children.push(LessMixinCalleeChild { name, combinator, span });
            } else {
                break;
            }
        }

        if let Some(last) = children.last() {
            span.end = last.span.end;
        }
        Ok(LessMixinCallee { children, span })
    }
}

impl<'a> Parse<'a> for LessMixinDefinition<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert_eq!(input.syntax, Syntax::Less);

        let name = input.parse::<LessMixinName>()?;

        let params = input.parse_less_mixin_parameters()?;

        let guard = match &peek!(input).token {
            Token::Ident(ident) if ident.raw == "when" => Some(input.parse()?),
            _ => None,
        };

        let block = input
            .with_state(ParserState {
                less_ctx: input.state.less_ctx | LESS_CTX_ALLOW_KEYFRAME_BLOCK,
                ..input.state.clone()
            })
            .parse::<SimpleBlock>()?;

        let span = Span { start: name.span().start, end: block.span.end };

        Ok(LessMixinDefinition { name, params, guard, block, span })
    }
}

impl<'a> Parse<'a> for LessMixinName<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        match bump!(input) {
            // `.3D` — a digit-led name arrives as one <dimension-token>
            TokenWithSpan { token: Token::Dimension(..), span }
                if input.source.as_bytes().get(span.start) == Some(&b'.') =>
            {
                let name_span = Span { start: span.start + 1, end: span.end };
                let raw = &input.source[name_span.start..name_span.end];
                Ok(LessMixinName::ClassSelector(ClassSelector {
                    name: InterpolableIdent::Literal(Ident { name: raw, raw, span: name_span }),
                    span,
                }))
            }
            TokenWithSpan { token: Token::Dot(..), span: dot_span } => {
                let (ident, ident_span) = expect_without_ws_or_comments!(input, Ident);
                let ident = input.ident(ident, ident_span);
                let span = Span { start: dot_span.start, end: ident.span.end };
                Ok(LessMixinName::ClassSelector(ClassSelector {
                    name: InterpolableIdent::Literal(ident),
                    span,
                }))
            }
            TokenWithSpan { token: Token::Hash(hash), span } => {
                let raw = hash.raw;
                if raw.starts_with(|c: char| c.is_ascii_digit())
                    || matches!(raw.as_bytes(), [b'-'] | [b'-', b'0'..=b'9', ..])
                {
                    input
                        .recoverable_errors
                        .push(Error { kind: ErrorKind::InvalidIdSelectorName, span: span.clone() });
                }
                let name =
                    if hash.escaped { util::handle_escape_in(raw, input.allocator()) } else { raw };
                let name_span = Span { start: span.start + 1, end: span.end };
                Ok(LessMixinName::IdSelector(IdSelector {
                    name: InterpolableIdent::Literal(Ident { name, raw, span: name_span }),
                    span,
                }))
            }
            TokenWithSpan { token, span } => {
                use crate::{
                    token::{Dot, Hash},
                    tokenizer::TokenSymbol,
                };
                Err(Error {
                    kind: ErrorKind::ExpectOneOf(
                        vec![Dot::symbol(), Hash::symbol()],
                        token.symbol(),
                    ),
                    span,
                })
            }
        }
    }
}

impl<'a> Parse<'a> for LessMixinParameterName<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        if matches!(peek!(input).token, Token::AtKeyword(..)) {
            input.parse().map(LessMixinParameterName::Variable)
        } else {
            input.parse().map(LessMixinParameterName::PropertyVariable)
        }
    }
}

impl<'a> Parse<'a> for LessNamespaceValue<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let callee = input.parse::<LessNamespaceValueCallee>()?;
        let callee_span = callee.span();

        let lookups = input.parse::<LessLookups>()?;
        util::assert_no_ws_or_comment(callee_span, &lookups.span)?;

        let span = Span { start: callee_span.start, end: lookups.span.end };
        Ok(LessNamespaceValue { callee, lookups, span })
    }
}

impl<'a> Parse<'a> for LessNamespaceValueCallee<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        if matches!(peek!(input).token, Token::AtKeyword(..)) {
            input.parse().map(LessNamespaceValueCallee::LessVariable)
        } else {
            input.parse().map(LessNamespaceValueCallee::LessMixinCall)
        }
    }
}

impl<'a> Parse<'a> for LessNegativeValue<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, minus_span) = expect!(input, Minus);
        let value = match peek!(input) {
            TokenWithSpan {
                token: Token::AtKeyword(..) | Token::At(..) | Token::DollarVar(..),
                span,
            } if minus_span.end == span.start => {
                let value = input.parse_component_value_atom()?;
                arena_box!(input, value)
            }
            TokenWithSpan { token: Token::LParen(..), span } if minus_span.end == span.start => {
                let value = ComponentValue::LessParenthesizedOperation(
                    input.parse_less_parenthesized_operation(/* allow_mixin_call */ true)?,
                );
                arena_box!(input, value)
            }
            TokenWithSpan { token, span } => {
                use crate::{
                    token::{AtKeyword, DollarVar, LParen},
                    tokenizer::TokenSymbol,
                };
                return Err(Error {
                    kind: ErrorKind::ExpectOneOf(
                        vec![AtKeyword::symbol(), DollarVar::symbol(), LParen::symbol()],
                        token.symbol(),
                    ),
                    span: span.clone(),
                });
            }
        };

        let span = Span { start: minus_span.start, end: value.span().end };
        Ok(LessNegativeValue { value, span })
    }
}

impl<'a> Parse<'a> for LessPercentKeyword {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, span) = expect!(input, Percent);
        Ok(LessPercentKeyword { span })
    }
}

impl<'a> Parse<'a> for LessPlugin<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert_eq!(input.syntax, Syntax::Less);

        let mut start = None;

        let args = if let Some((_, span)) = eat!(input, LParen) {
            start = Some(span.start);
            let args = input.parse_tokens_in_parens()?;
            expect!(input, RParen);
            Some(args)
        } else {
            None
        };

        let path = input.parse::<LessPluginPath>()?;
        let path_span = path.span();

        let span = Span { start: start.unwrap_or(path_span.start), end: path_span.end };
        Ok(LessPlugin { path, args, span })
    }
}

impl<'a> Parse<'a> for LessPluginPath<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        if let Token::Str(..) = peek!(input).token {
            input.parse().map(LessPluginPath::Str)
        } else {
            input.parse().map(LessPluginPath::Url)
        }
    }
}

impl<'a> Parse<'a> for LessPropertyInterpolation<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (dollar_lbrace_var, span) = expect!(input, DollarLBraceVar);
        Ok(LessPropertyInterpolation {
            name: input
                .ident(dollar_lbrace_var.ident, Span { start: span.start + 2, end: span.end - 1 }),
            span,
        })
    }
}

impl<'a> Parse<'a> for Option<LessPropertyMerge> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert!(matches!(input.syntax, Syntax::Less | Syntax::Css));

        match &peek!(input).token {
            Token::Plus(..) => Ok(Some(LessPropertyMerge {
                kind: LessPropertyMergeKind::Comma,
                span: bump!(input).span,
            })),
            Token::PlusUnderscore(..) => Ok(Some(LessPropertyMerge {
                kind: LessPropertyMergeKind::Space,
                span: bump!(input).span,
            })),
            _ => Ok(None),
        }
    }
}

impl<'a> Parse<'a> for LessPropertyVariable<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (dollar_var, span) = expect!(input, DollarVar);
        Ok(LessPropertyVariable {
            name: input.ident(dollar_var.ident, Span { start: span.start + 1, end: span.end }),
            span,
        })
    }
}

impl<'a> Parse<'a> for LessVariable<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (at_keyword, span) = expect!(input, AtKeyword);
        Ok(LessVariable {
            name: input.ident(at_keyword.ident, Span { start: span.start + 1, end: span.end }),
            span,
        })
    }
}

impl<'a> Parse<'a> for LessVariableCall<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let variable = input.parse::<LessVariable>()?;
        expect_without_ws_or_comments!(input, LParen);
        let (_, Span { end, .. }) = expect!(input, RParen);

        let span = Span { start: variable.span.start, end };
        Ok(LessVariableCall { variable, span })
    }
}

impl<'a> Parse<'a> for LessVariableDeclaration<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        debug_assert_eq!(input.syntax, Syntax::Less);

        let name = input.parse::<LessVariable>()?;
        let (_, colon_span) = expect!(input, Colon);
        let value = if matches!(peek!(input).token, Token::LBrace(..)) {
            ComponentValue::LessDetachedRuleset(input.parse()?)
        } else {
            let typed = input.try_parse(|p| {
                let value = p
                    .with_state(ParserState {
                        less_ctx: p.state.less_ctx | LESS_CTX_ALLOW_DIV,
                        ..p.state.clone()
                    })
                    .parse_maybe_less_list(/* allow_comma */ true)?;
                // The declaration must account for everything up to a
                // statement boundary — `@page :first {` is an at-rule, not a
                // variable named `@page` with the value `first`.
                if !matches!(
                    &peek!(p).token,
                    Token::Semicolon(..) | Token::RBrace(..) | Token::Eof(..)
                ) {
                    let span = peek!(p).span.clone();
                    return Err(Error { kind: ErrorKind::TryParseError, span });
                }
                Ok(value)
            });
            match typed {
                Ok(value) => value,
                Err(error) => {
                    // less.js `permissiveValue`: a variable's value may be any
                    // balanced token run (`@this: () => { ... };`), but only
                    // when explicitly terminated by `;` — otherwise
                    // `@page :first { ... }` would be swallowed too.
                    let start = peek!(input).span.start;
                    let values = input
                        .parse_declaration_value_tokens(/* stop_at_top_level_brace */ false)?;
                    let end = values.last().map(|v| v.span().end);
                    if !matches!(peek!(input).token, Token::Semicolon(..)) {
                        return Err(error);
                    }
                    let Some(end) = end else {
                        return Err(error);
                    };
                    ComponentValue::LessList(LessList {
                        elements: values,
                        comma_spans: None,
                        span: Span { start, end },
                    })
                }
            }
        };
        let span = Span { start: name.span.start, end: value.span().end };
        Ok(LessVariableDeclaration { name, colon_span, value, span })
    }
}

impl<'a> Parse<'a> for LessVariableInterpolation<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (at_lbrace_var, span) = expect!(input, AtLBraceVar);
        Ok(LessVariableInterpolation {
            name: input
                .ident(at_lbrace_var.ident, Span { start: span.start + 2, end: span.end - 1 }),
            span,
        })
    }
}

impl<'a> Parse<'a> for LessVariableVariable<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, at_span) = expect!(input, At);
        let variable = input.parse::<LessVariable>()?;
        util::assert_no_ws_or_comment(&at_span, &variable.span)?;

        let span = Span { start: at_span.start, end: variable.span.end };
        Ok(LessVariableVariable { variable, span })
    }
}

fn wrap_less_mixin_params_into_less_list<'a>(
    allocator: &'a oxc_allocator::Allocator,
    params: &mut oxc_allocator::Vec<'a, LessMixinParameter<'a>>,
    comma_spans: oxc_allocator::Vec<'a, Span>,
    index: usize,
) -> Result<(), ErrorKind> {
    if let [first, .., last] = &params[index..] {
        let span = Span { start: first.span().start, end: last.span().end };
        // `@margin: 2, 2, 2, 2; ...` — a named parameter followed by plain
        // values is one parameter whose default is the comma list. Two named
        // parameters in one comma group stay rejected
        // (`.mixin(@a: 5, @b: 6; @c: 7)`), as in less.js.
        let named_head = matches!(
            &params[index],
            LessMixinParameter::Named(LessMixinNamedParameter { value: Some(..), .. })
        ) && params.len() - index > 1;
        let mut drained = params.drain(index..);
        let head = if named_head { drained.next() } else { None };
        let mut elements = oxc_allocator::Vec::with_capacity_in(drained.len() + 1, &allocator);
        let head = match head {
            Some(LessMixinParameter::Named(LessMixinNamedParameter {
                name,
                value: Some(default),
                ..
            })) => {
                elements.push(default.value);
                Some((name, default.colon_span))
            }
            _ => None,
        };
        for param in drained {
            if let LessMixinParameter::Unnamed(LessMixinUnnamedParameter { value, .. }) = param {
                elements.push(value);
            } else {
                // reject code like this:
                // .mixin(@a: 5, @b: 6; @c: 7) {}
                // .mixin(@a: 5; @b: 6, @c: 7) {}
                return Err(ErrorKind::MixedDelimiterKindInLessMixin);
            }
        }
        debug_assert!(comma_spans.len() < elements.len());
        let list_span =
            Span { start: elements.first().map_or(span.start, |v| v.span().start), end: span.end };
        let list = ComponentValue::LessList(LessList {
            elements,
            comma_spans: Some(comma_spans),
            span: list_span.clone(),
        });
        params.push(match head {
            Some((name, colon_span)) => LessMixinParameter::Named(LessMixinNamedParameter {
                span: Span { start: name.span().start, end: span.end },
                name,
                value: Some(LessMixinNamedParameterDefaultValue {
                    colon_span,
                    value: list,
                    span: list_span,
                }),
            }),
            None => LessMixinParameter::Unnamed(LessMixinUnnamedParameter { value: list, span }),
        });
    }
    Ok(())
}

fn wrap_less_mixin_args_into_less_list<'a>(
    allocator: &'a oxc_allocator::Allocator,
    args: &mut oxc_allocator::Vec<'a, LessMixinArgument<'a>>,
    comma_spans: oxc_allocator::Vec<'a, Span>,
    index: usize,
) -> Result<(), ErrorKind> {
    if let [first, .., last] = &args[index..] {
        let span = Span { start: first.span().start, end: last.span().end };
        // `@a : d, e; @b : f` — a named argument followed by plain values is
        // one named argument whose value is the comma list; its trailing
        // values fold into it. Two named arguments in one comma group stay
        // rejected (`.mixin(@a: 5, @b: 6; @c: 7)`), as in less.js.
        let named_head =
            matches!(&args[index], LessMixinArgument::Named(..)) && args.len() - index > 1;
        let mut drained = args.drain(index..);
        let head = if named_head { drained.next() } else { None };
        let mut elements = oxc_allocator::Vec::with_capacity_in(drained.len() + 1, &allocator);
        let mut head = match head {
            Some(LessMixinArgument::Named(named)) => {
                elements.push(named.value);
                Some((named.name, named.colon_span))
            }
            _ => None,
        };
        for arg in drained {
            if let LessMixinArgument::Value(value) = arg {
                elements.push(value);
            } else {
                // reject code like this:
                // .mixin(@a: 5, @b: 6; @c: 7) {}
                // .mixin(@a: 5; @b: 6, @c: 7) {}
                return Err(ErrorKind::MixedDelimiterKindInLessMixin);
            }
        }
        debug_assert!(comma_spans.len() < elements.len());
        let list_span =
            Span { start: elements.first().map_or(span.start, |v| v.span().start), end: span.end };
        let list = ComponentValue::LessList(LessList {
            elements,
            comma_spans: Some(comma_spans),
            span: list_span,
        });
        args.push(match head.take() {
            Some((name, colon_span)) => LessMixinArgument::Named(LessMixinNamedArgument {
                span: Span { start: name.span().start, end: span.end },
                name,
                colon_span,
                value: list,
            }),
            None => LessMixinArgument::Value(list),
        });
    }
    Ok(())
}

fn can_be_division_operand(left: &ComponentValue) -> bool {
    matches!(
        left,
        ComponentValue::LessVariable(..)
            | ComponentValue::LessPropertyVariable(..)
            | ComponentValue::LessBinaryOperation(..)
            | ComponentValue::LessParenthesizedOperation(..)
    )
}

/// Whether the byte at `pos` in `source` is ASCII whitespace. Used to tell a
/// `+`/`-` binary operator (followed by whitespace) from a value sign.
fn is_followed_by_whitespace(source: &str, pos: usize) -> bool {
    source.as_bytes().get(pos).is_some_and(u8::is_ascii_whitespace)
}

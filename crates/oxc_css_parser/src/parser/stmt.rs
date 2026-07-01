use super::{
    Parser,
    state::{ParserState, QualifiedRuleContext},
};
use crate::{
    Parse, Syntax, arena_box, arena_vec,
    ast::*,
    bump, eat,
    error::{Error, ErrorKind, PResult},
    expect, peek,
    pos::{Span, Spanned},
    tokenizer::{Token, TokenWithSpan},
    util::PairedToken,
};

impl<'a> Parse<'a> for Declaration<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        // Legacy IE hack: a `*` glued to the property name (e.g. `*color: red`)
        // makes IE<=7 apply the declaration. Keep it as a property-name prefix.
        let name_prefix_start = if let Token::Asterisk(..) = peek!(input).token {
            Some(bump!(input).span.start)
        } else {
            None
        };
        // A css-in-js `${}` placeholder may stand in for the property name
        // (`${foo}: ${bar}`); it is not a real ident, so accept it directly.
        let name = if let Token::Placeholder(..) = peek!(input).token {
            let (placeholder, span) = expect!(input, Placeholder);
            InterpolableIdent::Placeholder((placeholder, span).into())
        } else {
            input
                .with_state(ParserState {
                    qualified_rule_ctx: Some(QualifiedRuleContext::DeclarationName),
                    ..input.state
                })
                .parse::<InterpolableIdent>()?
        };

        // https://tailwindcss.com/docs/theme#overriding-the-default-theme
        let name_suffix = if let TokenWithSpan { token: Token::Asterisk(..), span } = peek!(input)
            && name.span().end == span.start
        {
            bump!(input);
            Some('*')
        } else {
            None
        };

        let less_property_merge = if input.syntax == Syntax::Less { input.parse()? } else { None };

        let (_, colon_span) = expect!(input, Colon);
        let value = {
            let mut parser = input.with_state(ParserState {
                qualified_rule_ctx: Some(QualifiedRuleContext::DeclarationValue),
                ..input.state
            });
            match &name {
                InterpolableIdent::Literal(ident)
                    if ident.name.starts_with("--")
                        || matches!(
                            &peek!(parser).token,
                            // for IE-compatibility, regardless of the property
                            // name (`filter`, `-ms-filter`, vendor variants...):
                            // filter: progid:DXImageTransform.Microsoft...
                            Token::Ident(ident) if ident.name().eq_ignore_ascii_case("progid")
                        ) =>
                'value: {
                    if parser.options.try_parsing_value_in_custom_property
                        && let Ok(values) = parser.try_parse(Parser::parse_declaration_value)
                    {
                        break 'value values;
                    }

                    let mut values = parser.vec_with_capacity(3);
                    let mut pairs = Vec::with_capacity(1);
                    loop {
                        match &peek!(parser).token {
                            Token::Dedent(..) | Token::Linebreak(..) | Token::Eof(..) => break,
                            Token::Semicolon(..) if pairs.is_empty() => {
                                break;
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
                            // An interpolated string (e.g. `'#{$expr}'` inside
                            // `filter: progid:...`) must be parsed structurally:
                            // the tokenizer needs `scan_string_template` to resume
                            // the string after each `#{...}`, so consuming its
                            // tokens as a plain stream would mis-lex the rest.
                            Token::StrTemplate(..) => {
                                values.push(ComponentValue::InterpolableStr(parser.parse()?));
                                continue;
                            }
                            _ => {}
                        }
                        values.push(ComponentValue::TokenWithSpan(bump!(parser)));
                    }
                    values
                }
                _ => parser.parse_declaration_value()?,
            }
        };

        let important = if let Token::Exclamation(..) = &peek!(input).token {
            input.parse::<ImportantAnnotation>().map(Some)?
        } else {
            None
        };

        let span = Span {
            start: name_prefix_start.unwrap_or(name.span().start),
            end: if let Some(important) = &important {
                important.span.end
            } else if let Some(last) = value.last() {
                last.span().end
            } else {
                colon_span.end
            },
        };
        Ok(Declaration {
            name,
            name_prefix: name_prefix_start.map(|_| '*'),
            name_suffix,
            colon_span,
            value,
            important,
            less_property_merge,
            span,
        })
    }
}

impl<'a> Parse<'a> for ImportantAnnotation<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, span) = expect!(input, Exclamation);
        let ident: Ident = input.parse::<Ident>()?;
        let span = Span { start: span.start, end: ident.span.end };
        if ident.name.eq_ignore_ascii_case("important") {
            Ok(ImportantAnnotation { ident, span })
        } else {
            Err(Error { kind: ErrorKind::ExpectImportantAnnotation, span })
        }
    }
}

impl<'a> Parse<'a> for QualifiedRule<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let selector_list = input
            .with_state(ParserState {
                qualified_rule_ctx: Some(QualifiedRuleContext::Selector),
                ..input.state
            })
            .parse::<SelectorList>()?;
        let block = input.parse::<SimpleBlock>()?;
        let span = Span { start: selector_list.span.start, end: block.span.end };
        Ok(QualifiedRule { selector: selector_list, block, span })
    }
}

impl<'a> Parse<'a> for SimpleBlock<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let is_sass = input.syntax == Syntax::Sass;
        let start = if is_sass {
            if let Some((_, span)) = eat!(input, Indent) {
                span.end
            } else {
                let offset = peek!(input).span.start;
                return Ok(SimpleBlock {
                    statements: arena_vec!(input),
                    span: Span { start: offset, end: offset },
                });
            }
        } else {
            expect!(input, LBrace).1.start
        };

        let statements = input.parse_statements(/* is_top_level */ false)?;

        if is_sass {
            match bump!(input) {
                TokenWithSpan { token: Token::Dedent(..) | Token::Eof(..), span } => {
                    let end = statements.last().map_or(span.start, |last| last.span().end);
                    Ok(SimpleBlock { statements, span: Span { start, end } })
                }
                TokenWithSpan { span, .. } => {
                    Err(Error { kind: ErrorKind::ExpectDedentOrEof, span })
                }
            }
        } else {
            let end = expect!(input, RBrace).1.end;
            Ok(SimpleBlock { statements, span: Span { start, end } })
        }
    }
}

impl<'a> Parse<'a> for Stylesheet<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let statements = input.parse_statements(/* is_top_level */ true)?;
        expect!(input, Eof);
        Ok(Stylesheet { statements, span: Span { start: 0, end: input.source.len() } })
    }
}

impl<'a> Parser<'a> {
    pub(super) fn parse_declaration_value(
        &mut self,
    ) -> PResult<oxc_allocator::Vec<'a, ComponentValue<'a>>> {
        let mut values = self.vec_with_capacity(3);
        loop {
            match &peek!(self).token {
                Token::RBrace(..)
                | Token::RParen(..)
                | Token::Semicolon(..)
                | Token::Dedent(..)
                | Token::Linebreak(..)
                | Token::Exclamation(..)
                | Token::Eof(..) => break,
                _ => {
                    let value = self.parse::<ComponentValue>()?;
                    match &value {
                        ComponentValue::SassNestingDeclaration(..)
                            if matches!(self.syntax, Syntax::Scss | Syntax::Sass) =>
                        {
                            values.push(value);
                            break;
                        }
                        _ => values.push(value),
                    }
                }
            }
        }
        Ok(values)
    }

    fn parse_statements(
        &mut self,
        is_top_level: bool,
    ) -> PResult<oxc_allocator::Vec<'a, Statement<'a>>> {
        let mut statements = self.vec_with_capacity(1);
        loop {
            // Set true for braced blocks AND `${}` placeholder statements: both
            // make the trailing terminator optional. A placeholder substitutes a
            // whole statement/declaration and, like postcss, needs no `;`, so the
            // next statement may follow directly (`${mixin}\n@media {...}`,
            // `${a} ${b}`, `${foo}: ${bar}`).
            let mut is_block_element = false;
            let TokenWithSpan { token, span } = peek!(self);
            match token {
                Token::Ident(..) | Token::HashLBrace(..) | Token::AtLBraceVar(..) => {
                    match self.syntax {
                        Syntax::Css => {
                            if self.state.in_keyframes_at_rule {
                                statements.push(Statement::KeyframeBlock(self.parse()?));
                                is_block_element = true;
                            } else {
                                match self.try_parse(QualifiedRule::parse) {
                                    Ok(rule) => {
                                        statements.push(Statement::QualifiedRule(rule));
                                        is_block_element = true;
                                    }
                                    Err(error_rule) => match self.parse::<Declaration>() {
                                        Ok(decl) => {
                                            if is_top_level {
                                                self.recoverable_errors.push(Error {
                                                    kind: ErrorKind::TopLevelDeclaration,
                                                    span: decl.span.clone(),
                                                });
                                            }
                                            statements.push(Statement::Declaration(decl));
                                        }
                                        Err(error_decl) => {
                                            if is_top_level {
                                                return Err(error_rule);
                                            } else {
                                                return Err(error_decl);
                                            }
                                        }
                                    },
                                }
                            }
                        }
                        Syntax::Scss | Syntax::Sass => {
                            if let Ok(sass_var_decl) =
                                self.try_parse(SassVariableDeclaration::parse)
                            {
                                statements.push(Statement::SassVariableDeclaration(arena_box!(
                                    self,
                                    sass_var_decl
                                )));
                            } else if self.state.in_keyframes_at_rule {
                                statements.push(Statement::KeyframeBlock(self.parse()?));
                                is_block_element = true;
                            } else {
                                match self.try_parse(QualifiedRule::parse) {
                                    Ok(rule) => {
                                        statements.push(Statement::QualifiedRule(rule));
                                        is_block_element = true;
                                    }
                                    Err(error_rule) => match self.parse::<Declaration>() {
                                        Ok(decl) => {
                                            is_block_element = matches!(
                                                decl.value.last(),
                                                Some(ComponentValue::SassNestingDeclaration(..))
                                            );
                                            if is_top_level {
                                                self.recoverable_errors.push(Error {
                                                    kind: ErrorKind::TopLevelDeclaration,
                                                    span: decl.span.clone(),
                                                });
                                            }
                                            statements.push(Statement::Declaration(decl));
                                        }
                                        Err(error_decl) => {
                                            if is_top_level {
                                                return Err(error_rule);
                                            } else {
                                                return Err(error_decl);
                                            }
                                        }
                                    },
                                }
                            }
                        }
                        Syntax::Less => {
                            if let Ok(stmt) = self.try_parse(Parser::parse_less_qualified_rule) {
                                statements.push(stmt);
                                is_block_element = true;
                            } else if let Ok(decl) = self.try_parse(|parser| {
                                if is_top_level {
                                    Err(Error {
                                        kind: ErrorKind::TryParseError,
                                        span: bump!(parser).span,
                                    })
                                } else {
                                    parser.parse()
                                }
                            }) {
                                statements.push(Statement::Declaration(decl));
                            } else if self.state.in_keyframes_at_rule {
                                statements.push(Statement::KeyframeBlock(self.parse()?));
                                is_block_element = true;
                            } else {
                                let fn_call = self.parse::<Function>()?;
                                is_block_element = matches!(
                                    fn_call.args.last(),
                                    Some(ComponentValue::LessDetachedRuleset(..))
                                );
                                statements.push(Statement::LessFunctionCall(fn_call));
                            }
                        }
                    }
                }
                Token::Dot(..) | Token::Hash(..) if self.syntax == Syntax::Less => {
                    let stmt = if let Ok(stmt) = self.try_parse(Parser::parse_less_qualified_rule) {
                        is_block_element = true;
                        stmt
                    } else if let Ok(mixin_def) = self.try_parse(LessMixinDefinition::parse) {
                        is_block_element = true;
                        Statement::LessMixinDefinition(arena_box!(self, mixin_def))
                    } else {
                        self.parse().map(Statement::LessMixinCall)?
                    };
                    statements.push(stmt);
                }
                Token::Dot(..) | Token::Hash(..) if !self.state.in_keyframes_at_rule => {
                    statements.push(Statement::QualifiedRule(self.parse()?));
                    is_block_element = true;
                }
                Token::Ampersand(..)
                | Token::LBracket(..)
                | Token::Colon(..)
                | Token::ColonColon(..)
                | Token::Asterisk(..)
                | Token::Bar(..)
                | Token::NumberSign(..)
                    if !self.state.in_keyframes_at_rule =>
                {
                    if self.syntax == Syntax::Less {
                        if let Ok(extend_rule) = self.try_parse(LessExtendRule::parse) {
                            statements.push(Statement::LessExtendRule(extend_rule));
                        } else if matches!(peek!(self).token, Token::Asterisk(..)) {
                            // `*zoom: 1` (an IE<=7 hack) looks like a `*` universal
                            // selector but is a declaration; try the rule, then fall back.
                            if let Ok(stmt) = self.try_parse(Parser::parse_less_qualified_rule) {
                                statements.push(stmt);
                                is_block_element = true;
                            } else {
                                let decl = self.parse::<Declaration>()?;
                                statements.push(Statement::Declaration(decl));
                            }
                        } else {
                            statements.push(self.parse_less_qualified_rule()?);
                            is_block_element = true;
                        }
                    } else if matches!(peek!(self).token, Token::Asterisk(..)) {
                        // `*color: red` (an IE<=7 hack) looks like a `*` universal
                        // selector but is a declaration; try the rule, then fall back
                        // to a declaration before surfacing the selector error.
                        match self.try_parse(QualifiedRule::parse) {
                            Ok(rule) => {
                                statements.push(Statement::QualifiedRule(rule));
                                is_block_element = true;
                            }
                            Err(error_rule) => match self.parse::<Declaration>() {
                                Ok(decl) => {
                                    if is_top_level {
                                        self.recoverable_errors.push(Error {
                                            kind: ErrorKind::TopLevelDeclaration,
                                            span: decl.span.clone(),
                                        });
                                    }
                                    statements.push(Statement::Declaration(decl));
                                }
                                Err(error_decl) => {
                                    return Err(if is_top_level { error_rule } else { error_decl });
                                }
                            },
                        }
                    } else {
                        statements.push(Statement::QualifiedRule(self.parse()?));
                        is_block_element = true;
                    }
                }
                Token::AtKeyword(at_keyword) => match self.syntax {
                    Syntax::Css => {
                        let at_rule = self.parse::<AtRule>()?;
                        is_block_element = at_rule.block.is_some();
                        statements.push(Statement::AtRule(at_rule));
                    }
                    Syntax::Scss | Syntax::Sass => {
                        let at_keyword_name = at_keyword.ident.name();
                        match &*at_keyword_name {
                            "if" => {
                                statements
                                    .push(Statement::SassIfAtRule(arena_box!(self, self.parse()?)));
                                is_block_element = true;
                            }
                            "else" => {
                                return Err(Error {
                                    kind: ErrorKind::UnexpectedSassElseAtRule,
                                    span: bump!(self).span,
                                });
                            }
                            _ => {
                                let at_rule = self.parse::<AtRule>()?;
                                is_block_element = at_rule.block.is_some();
                                statements.push(Statement::AtRule(at_rule));
                            }
                        }
                    }
                    Syntax::Less => {
                        if let Ok(less_variable_declaration) =
                            self.try_parse(LessVariableDeclaration::parse)
                        {
                            is_block_element = matches!(
                                less_variable_declaration.value,
                                ComponentValue::LessDetachedRuleset(..)
                            );
                            statements.push(Statement::LessVariableDeclaration(arena_box!(
                                self,
                                less_variable_declaration
                            )));
                        } else if let Ok(variable_call) = self.try_parse(LessVariableCall::parse) {
                            statements.push(Statement::LessVariableCall(variable_call));
                        } else {
                            let at_rule = self.parse::<AtRule>()?;
                            is_block_element = at_rule.block.is_some();
                            statements.push(Statement::AtRule(at_rule));
                        }
                    }
                },
                Token::Placeholder(..) => {
                    // A placeholder may start a qualified rule (a substituted
                    // selector, e.g. CSS-in-JS `${Component} { ... }`) or stand
                    // alone as a statement (e.g. `` `PLACEHOLDER-0`; ``).
                    //
                    // A placeholder-led selector must not absorb across a newline:
                    // prettier keeps `${mixin}` on its own line and the following
                    // selector as a separate rule (`${mixin}\n& > .x {}` is two
                    // statements, not one). So only attempt the rule when the block
                    // `{` is reachable without an intervening newline-then-selector.
                    //
                    // A placeholder may also be a declaration property name
                    // (`${foo}: ${bar}`), so try a declaration before falling back
                    // to a bare placeholder statement.
                    let ph_end = peek!(self).span.end;
                    if self.placeholder_starts_qualified_rule(ph_end)
                        && let Ok(rule) = self.try_parse(QualifiedRule::parse)
                    {
                        statements.push(Statement::QualifiedRule(rule));
                        is_block_element = true;
                    } else if let Ok(declaration) = self.try_parse(Declaration::parse) {
                        // Reached only via the placeholder token above, so this
                        // is the `${foo}: ${bar}` form (placeholder property name).
                        statements.push(Statement::Declaration(declaration));
                        is_block_element = true;
                    } else {
                        let (placeholder, span) = expect!(self, Placeholder);
                        statements.push(Statement::Placeholder((placeholder, span).into()));
                        is_block_element = true;
                    }
                }
                Token::Percent(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                    statements.push(Statement::QualifiedRule(self.parse()?));
                    is_block_element = true;
                }
                Token::DollarVar(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                    statements
                        .push(Statement::SassVariableDeclaration(arena_box!(self, self.parse()?)));
                }
                Token::DollarVar(..)
                    if self.syntax == Syntax::Css && self.options.allow_postcss_simple_vars =>
                {
                    statements.push(Statement::PostcssSimpleVarDeclaration(arena_box!(
                        self,
                        self.parse()?
                    )));
                }
                Token::GreaterThan(..) | Token::Plus(..) | Token::Tilde(..) | Token::BarBar(..) => {
                    if self.syntax == Syntax::Less {
                        statements.push(self.parse_less_qualified_rule()?);
                    } else {
                        statements.push(Statement::QualifiedRule(self.parse()?));
                    }
                    is_block_element = true;
                }
                Token::DollarLBraceVar(..) if self.syntax == Syntax::Less => {
                    statements.push(self.parse().map(Statement::Declaration)?);
                }
                Token::Cdo(..) | Token::Cdc(..) => {
                    bump!(self);
                    continue;
                }
                Token::At(..) if matches!(self.syntax, Syntax::Scss | Syntax::Sass) => {
                    let unknown_sass_at_rule = self.parse::<UnknownSassAtRule>()?;
                    is_block_element = unknown_sass_at_rule.block.is_some();
                    statements
                        .push(Statement::UnknownSassAtRule(arena_box!(self, unknown_sass_at_rule)));
                }
                Token::Percentage(..)
                    if self.state.in_keyframes_at_rule
                        || self.state.sass_ctx & super::state::SASS_CTX_ALLOW_KEYFRAME_BLOCK
                            != 0
                        || self.state.less_ctx & super::state::LESS_CTX_ALLOW_KEYFRAME_BLOCK
                            != 0 =>
                {
                    statements.push(Statement::KeyframeBlock(self.parse()?));
                    is_block_element = true;
                }
                Token::RBrace(..) | Token::Eof(..) | Token::Dedent(..) => break,
                Token::Semicolon(..) | Token::Linebreak(..) => {
                    bump!(self);
                    continue;
                }
                _ => {
                    return Err(Error {
                        kind: if self.state.in_keyframes_at_rule {
                            ErrorKind::ExpectKeyframeBlock
                        } else {
                            ErrorKind::ExpectRule
                        },
                        span: span.clone(),
                    });
                }
            };
            match &peek!(self).token {
                Token::RBrace(..) | Token::Eof(..) | Token::Dedent(..) => break,
                _ => {
                    if self.syntax == Syntax::Sass {
                        if is_block_element {
                            eat!(self, Linebreak);
                        } else if self.options.tolerate_semicolon_in_sass {
                            if let Some((_, span)) = eat!(self, Semicolon) {
                                self.recoverable_errors.push(Error {
                                    kind: ErrorKind::UnexpectedSemicolonInSass,
                                    span,
                                });
                            }
                        } else {
                            expect!(self, Linebreak);
                        }
                    } else if is_block_element {
                        eat!(self, Semicolon);
                    } else {
                        expect!(self, Semicolon);
                    }
                }
            }
        }
        Ok(statements)
    }

    /// Whether a statement-position `${}` placeholder (ending at byte `from`)
    /// should be offered to `QualifiedRule::parse`. The css-in-js rule the parser
    /// can't see on its own, matching prettier:
    /// - a bare `{` after the placeholder IS absorbed — the placeholder is the
    ///   selector for that block (`${mixin}\n{ color: red }` is one rule; a bare
    ///   `{...}` is meaningless without a selector, so this is the only valid read)
    /// - a placeholder separated by whitespace from what follows, then a newline,
    ///   then selector content = a separate rule (`${mixin}\n& > .x {}` and
    ///   `${a} ${b}\nhtml {}` are two statements, not one — spaced placeholders
    ///   are typically mixin invocations, not selector pieces)
    /// - but a placeholder IMMEDIATELY glued to non-whitespace (e.g. `${p}:hover`
    ///   or `${p},`) is a compound-selector piece, so a multi-line selector list
    ///   (`${p}:hover &,\n${q}:focus &, { ... }`) is one rule — keep scanning for `{` across newlines.
    ///
    /// The real grammar (strings, comments, `#{...}` interpolations, validity) is
    /// left to `QualifiedRule::parse`, which runs next and rolls back if this guess was wrong.
    /// Deliberately NOT a tokenizer: it never early-exits on `;`/`}`
    /// (those may sit inside an attribute string or comment),
    /// so it can't misclassify a same-line selector containing them.
    fn placeholder_starts_qualified_rule(&self, from: usize) -> bool {
        let bytes = &self.source.as_bytes()[from..];
        // Immediately-adjacent non-whitespace (`${p}:hover`, `${p},`) means the
        // placeholder is a compound-selector piece: only `{` matters from here, regardless of newlines.
        if bytes.first().is_some_and(|b| !b.is_ascii_whitespace()) {
            return bytes.contains(&b'{');
        }
        // Otherwise the placeholder is separated by whitespace from what follows.
        // A `{` on the same line (whitespace-only prefix) still makes the
        // placeholder its selector; any non-whitespace after a newline starts a separate rule.
        let mut newline_seen = false;
        for &b in bytes {
            match b {
                b'{' => return true,
                // `\r`, `\r\n`, and `\n` all count as a newline (the tokenizer
                // treats a bare `\r` as a line break too).
                b'\n' | b'\r' => newline_seen = true,
                _ if b.is_ascii_whitespace() => {}
                _ if newline_seen => return false,
                _ => {}
            }
        }
        // No block at all -> a declaration or a bare placeholder, not a rule.
        false
    }
}

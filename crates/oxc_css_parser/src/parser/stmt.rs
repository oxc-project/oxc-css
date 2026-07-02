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
};

impl<'a> Parse<'a> for Declaration<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        // Legacy IE hack: a `*` glued to the property name (e.g. `*color: red`)
        // makes IE<=7 apply the declaration. Keep it as a property-name prefix — but
        // only when glued: `* color` (whitespace or a comment after `*`) is not the
        // hack, so leave the `*` for the normal (failing) parse.
        let name_prefix_start = if input.state.allow_ie_star_hack
            && let TokenWithSpan { token: Token::Asterisk(..), span } = peek!(input)
            && input
                .source
                .as_bytes()
                .get(span.end)
                .is_some_and(|b| !b.is_ascii_whitespace() && *b != b'/')
        {
            let start = span.start;
            bump!(input);
            Some(start)
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
        let (mut value, mut important) = {
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
                        break 'value (values, None);
                    }
                    (parser.parse_declaration_value_tokens(false)?, None)
                }
                // In CSS, a declaration value is any sequence of component
                // values (CSS Syntax §5): serialized selectors (`b: .c > d`),
                // map-like blocks (`b: (3: 4)`), or stray delimiters are all
                // valid preserved tokens even though the typed grammar has no
                // node for them. Try the typed grammar first; if it fails, or
                // succeeds without accounting for everything up to the
                // declaration terminator, re-parse the whole value as raw
                // tokens. Scss/Sass/Less keep the strict grammar: their
                // dialects assign meaning to these tokens and are expected to
                // reject exactly what their reference compilers reject.
                _ if parser.syntax == Syntax::Css
                    || (parser.state.in_css_function_body
                        && matches!(&name, InterpolableIdent::Literal(..))) =>
                {
                    let typed = parser.try_parse(|p| {
                        let values = p.parse_declaration_value()?;
                        let important = match &peek!(p).token {
                            Token::Exclamation(..) => Some(p.parse::<ImportantAnnotation>()?),
                            _ => None,
                        };
                        let next = peek!(p);
                        if at_declaration_value_end(&next.token) {
                            Ok((values, important))
                        } else {
                            Err(Error {
                                kind: ErrorKind::ExpectComponentValue,
                                span: next.span.clone(),
                            })
                        }
                    });
                    match typed {
                        Ok(value_and_important) => value_and_important,
                        Err(error) => {
                            // A CSS custom function body holds declarations
                            // only, so a top-level `{}` there is part of the
                            // value; elsewhere it means this construct is
                            // really a qualified rule (CSS Nesting
                            // disambiguation) and the declaration is rejected.
                            let in_fn_body = parser.state.in_css_function_body;
                            let values = parser.parse_declaration_value_tokens(!in_fn_body)?;
                            if !in_fn_body && let Token::LBrace(..) = &peek!(parser).token {
                                return Err(error);
                            }
                            (values, None)
                        }
                    }
                }
                _ => (parser.parse_declaration_value()?, None),
            }
        };

        if important.is_none()
            && let Token::Exclamation(..) = &peek!(input).token
        {
            important = Some(input.parse::<ImportantAnnotation>()?);
        }
        // dart-sass allows `!important` mid-value (`fludge: foo bar
        // !important hux;`): when more value follows, the annotation is just
        // another component, and only a trailing one is structural.
        while matches!(input.syntax, Syntax::Scss | Syntax::Sass)
            && important.is_some()
            && !at_declaration_value_end(&peek!(input).token)
        {
            if let Some(annotation) = important.take() {
                value.push(ComponentValue::ImportantAnnotation(annotation));
            }
            let more = input
                .with_state(ParserState {
                    qualified_rule_ctx: Some(QualifiedRuleContext::DeclarationValue),
                    ..input.state
                })
                .parse_declaration_value()?;
            for component in more {
                value.push(component);
            }
            if let Token::Exclamation(..) = &peek!(input).token {
                important = Some(input.parse::<ImportantAnnotation>()?);
            }
        }

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

/// End of a declaration's value: the declaration terminator tokens.
fn at_declaration_value_end(token: &Token) -> bool {
    matches!(
        token,
        Token::Semicolon(..)
            | Token::RBrace(..)
            | Token::RParen(..)
            | Token::Dedent(..)
            | Token::Linebreak(..)
            | Token::Eof(..)
    )
}

impl<'a> Parse<'a> for ImportantAnnotation<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (_, span) = expect!(input, Exclamation);
        input.eat_sass_line_continuation()?;
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
            // A continuation line deeper than this block's own level leaves a
            // pending indent whose `Dedent` arrives before the block opens
            // (`a,\n    b\n  c: d`); cancel those out first.
            let drained = input.drain_sass_pending_dedents()?;
            if let Some((_, span)) = eat!(input, Indent) {
                span.end
            } else if drained
                && input.sass_pending_indents == 0
                && input.tokenizer.reopen_indent_level()
            {
                // The block's level sat between two known indents, so its
                // `Indent` was never emitted; re-open it directly.
                peek!(input).span.start
            } else if input.sass_pending_indents > 0 {
                // The statement's clause consumed this block's `Indent` as a
                // line continuation (`@each $a in\n  b, c\n  .x\n    ...`);
                // enter the block "virtually" at that depth.
                input.sass_pending_indents -= 1;
                peek!(input).span.start
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
    /// Consume a declaration value as raw tokens (CSS Syntax "preserved
    /// tokens"), balancing `()`/`[]`/`{}` pairs, until a top-level `;`, an
    /// unbalanced closer, or a statement boundary. Used for custom-property
    /// values and as the fallback for CSS values the typed grammar rejects.
    ///
    /// `stop_at_top_level_brace` implements the CSS Nesting disambiguation: a
    /// `{` at the top level of a normal declaration's value means the whole
    /// construct is really a qualified rule, so the value must end there.
    /// Custom properties are exempt (`--foo: {a:b}` is a valid value).
    pub(super) fn parse_declaration_value_tokens(
        &mut self,
        stop_at_top_level_brace: bool,
    ) -> PResult<oxc_allocator::Vec<'a, ComponentValue<'a>>> {
        let mut values = self.vec_with_capacity(3);
        let mut pairs = Vec::with_capacity(1);
        loop {
            match &peek!(self).token {
                Token::Dedent(..) | Token::Linebreak(..) | Token::Eof(..) => break,
                Token::Semicolon(..) if pairs.is_empty() => {
                    break;
                }
                Token::LBrace(..) if stop_at_top_level_brace && pairs.is_empty() => {
                    break;
                }
                // An interpolated string (e.g. `'#{$expr}'` inside
                // `filter: progid:...`) must be parsed structurally:
                // the tokenizer needs `scan_string_template` to resume
                // the string after each `#{...}`, so consuming its
                // tokens as a plain stream would mis-lex the rest.
                Token::StrTemplate(..) => {
                    values.push(ComponentValue::InterpolableStr(self.parse()?));
                    continue;
                }
                token => {
                    if !crate::util::track_paired_token(token, &mut pairs) {
                        break;
                    }
                }
            }
            values.push(ComponentValue::TokenWithSpan(bump!(self)));
        }
        Ok(values)
    }

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

    /// In a `@keyframes` body, an ident may start a keyframe block (`from {`)
    /// or — in real-world code — a plain declaration (`blah: blee;`); dart-sass
    /// accepts both. Returns the statement and whether it opened a block.
    fn parse_keyframe_block_or_declaration(&mut self) -> PResult<(Statement<'a>, bool)> {
        if let Ok(block) = self.try_parse(KeyframeBlock::parse) {
            Ok((Statement::KeyframeBlock(block), true))
        } else {
            let decl = self.parse_style_rule_declaration()?;
            Ok((Statement::Declaration(decl), false))
        }
    }

    /// Parse a qualified rule, falling back to a declaration when the `foo: bar`
    /// vs `foo { }` prelude is ambiguous. Returns the statement and whether it
    /// opened a block (for the caller's `is_block_element`).
    fn parse_rule_or_declaration(&mut self, is_top_level: bool) -> PResult<(Statement<'a>, bool)> {
        match self.try_parse(QualifiedRule::parse) {
            Ok(rule) => Ok((Statement::QualifiedRule(rule), true)),
            Err(error_rule) => match self.parse_style_rule_declaration() {
                Ok(decl) => {
                    // Only Scss/Sass produce `SassNestingDeclaration`; in CSS this is
                    // always `false`, matching the previous per-syntax behavior.
                    let is_block_element = matches!(
                        decl.value.last(),
                        Some(ComponentValue::SassNestingDeclaration(..))
                    );
                    if is_top_level {
                        self.recoverable_errors.push(Error {
                            kind: ErrorKind::TopLevelDeclaration,
                            span: decl.span.clone(),
                        });
                    }
                    Ok((Statement::Declaration(decl), is_block_element))
                }
                Err(error_decl) => Err(if is_top_level { error_rule } else { error_decl }),
            },
        }
    }

    /// Parse a declaration that is a statement in a style-rule block, enabling the
    /// IE `*color` hack (see `ParserState::allow_ie_star_hack`). Feature-query
    /// declarations (`@supports`, `@container style()`, `@import supports()`) call
    /// `Declaration::parse` directly and so never enable it.
    fn parse_style_rule_declaration(&mut self) -> PResult<Declaration<'a>> {
        self.with_state(ParserState { allow_ie_star_hack: true, ..self.state.clone() }).parse()
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
                                let (stmt, is_block) =
                                    self.parse_keyframe_block_or_declaration()?;
                                is_block_element = is_block;
                                statements.push(stmt);
                            } else {
                                let (stmt, is_block) =
                                    self.parse_rule_or_declaration(is_top_level)?;
                                is_block_element = is_block;
                                statements.push(stmt);
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
                                let (stmt, is_block) =
                                    self.parse_keyframe_block_or_declaration()?;
                                is_block_element = is_block;
                                statements.push(stmt);
                            } else {
                                let (stmt, is_block) =
                                    self.parse_rule_or_declaration(is_top_level)?;
                                is_block_element = is_block;
                                statements.push(stmt);
                            }
                        }
                        Syntax::Less => {
                            if let Ok(stmt) = self.try_parse(Parser::parse_less_qualified_rule) {
                                statements.push(stmt);
                                is_block_element = true;
                            } else if let Ok(decl) =
                                // less.js parses root-level declarations and
                                // only rejects them at eval time.
                                self.try_parse(Declaration::parse)
                            {
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
                    if matches!(peek!(self).token, Token::Asterisk(..)) {
                        // `*color: red` / `*zoom: 1` (an IE<=7 hack) looks like a `*`
                        // universal selector but is a declaration; try the rule, then
                        // fall back to a declaration. (A `*` never starts a
                        // `LessExtendRule`, so this can precede the Less split.)
                        if self.syntax == Syntax::Less {
                            match self.try_parse(Parser::parse_less_qualified_rule) {
                                Ok(stmt) => {
                                    statements.push(stmt);
                                    is_block_element = true;
                                }
                                // Less refuses declarations at the top level, like the
                                // ident-led path; keep root-level `*zoom: 1` an error.
                                Err(rule_err) if is_top_level => return Err(rule_err),
                                Err(_) => {
                                    let decl = self.parse_style_rule_declaration()?;
                                    statements.push(Statement::Declaration(decl));
                                }
                            }
                        } else {
                            let (stmt, is_block) = self.parse_rule_or_declaration(is_top_level)?;
                            is_block_element = is_block;
                            statements.push(stmt);
                        }
                    } else if self.syntax == Syntax::Less {
                        if let Ok(extend_rule) = self.try_parse(LessExtendRule::parse) {
                            statements.push(Statement::LessExtendRule(extend_rule));
                        } else {
                            statements.push(self.parse_less_qualified_rule()?);
                            is_block_element = true;
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
                // Indented-syntax shorthands: `=name` defines a mixin
                // (`@mixin name`) and `+name` includes one (`@include name`).
                // A spaced `+ b` stays a sibling-combinator selector: `+` is
                // an include only when glued to an identifier.
                Token::Equal(..) if self.syntax == Syntax::Sass => {
                    let eq_span = bump!(self).span;
                    self.eat_sass_line_continuation()?;
                    let prelude = self.parse::<SassMixin>()?;
                    let block = self
                        .with_state(ParserState {
                            sass_ctx: self.state.sass_ctx
                                | super::state::SASS_CTX_ALLOW_KEYFRAME_BLOCK,
                            ..self.state.clone()
                        })
                        .parse::<SimpleBlock>()?;
                    let span = Span { start: eq_span.start, end: block.span.end };
                    statements.push(Statement::AtRule(AtRule {
                        name: Ident { name: "mixin", raw: "=", span: eq_span },
                        prelude: Some(AtRulePrelude::SassMixin(arena_box!(self, prelude))),
                        block: Some(block),
                        span,
                    }));
                    is_block_element = true;
                }
                Token::Plus(..)
                    if self.syntax == Syntax::Sass
                        && crate::tokenizer::ident_starts_at(self.source, span.end) =>
                {
                    let plus_span = bump!(self).span;
                    let prelude = self.parse::<SassInclude>()?;
                    let block =
                        if matches!(peek!(self).token, Token::LBrace(..) | Token::Indent(..)) {
                            Some(
                                self.with_state(ParserState {
                                    sass_ctx: self.state.sass_ctx
                                        | super::state::SASS_CTX_ALLOW_KEYFRAME_BLOCK,
                                    ..self.state.clone()
                                })
                                .parse::<SimpleBlock>()?,
                            )
                        } else {
                            None
                        };
                    let end = block.as_ref().map_or(prelude.span.end, |block| block.span.end);
                    let span = Span { start: plus_span.start, end };
                    is_block_element = block.is_some();
                    statements.push(Statement::AtRule(AtRule {
                        name: Ident { name: "include", raw: "+", span: plus_span },
                        prelude: Some(AtRulePrelude::SassInclude(arena_box!(self, prelude))),
                        block,
                        span,
                    }));
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
                Token::LBrace(..) if self.syntax == Syntax::Css => {
                    // An empty selector (`{}`): postcss parses it as a qualified rule
                    // with no selector, so build one with an empty selector list.
                    let start = span.start;
                    let block = self.parse::<SimpleBlock>()?;
                    let selector = SelectorList {
                        selectors: arena_vec!(self),
                        comma_spans: arena_vec!(self),
                        span: Span { start, end: start },
                    };
                    let span = Span { start, end: block.span.end };
                    statements.push(Statement::QualifiedRule(QualifiedRule {
                        selector,
                        block,
                        span,
                    }));
                    is_block_element = true;
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
            // Drain continuation indents that never became a block (e.g.
            // `$a\n  : b` — the deeper line belonged to the statement's own
            // clause, so its matching `Dedent` has no block to close). A
            // drained `Dedent` is itself a line boundary, so the statement
            // separator is already satisfied.
            if self.drain_sass_pending_dedents()? {
                continue;
            }
            match &peek!(self).token {
                Token::RBrace(..) | Token::Eof(..) | Token::Dedent(..) => break,
                _ => {
                    if self.syntax == Syntax::Sass {
                        // The indented syntax also accepts `;` as a statement
                        // terminator/separator (`a; b`), like a newline.
                        if is_block_element {
                            if eat!(self, Semicolon).is_none() {
                                eat!(self, Linebreak);
                            }
                        } else if eat!(self, Semicolon).is_none() {
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

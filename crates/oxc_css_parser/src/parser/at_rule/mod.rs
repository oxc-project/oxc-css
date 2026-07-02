use super::{Parser, state::ParserState};
use crate::{
    Parse, Syntax, arena_box,
    ast::*,
    bump,
    error::{Error, ErrorKind, PResult},
    expect, peek,
    pos::{Span, Spanned},
    tokenizer::Token,
};

mod color_profile;
mod container;
mod counter_style;
mod custom_media;
mod custom_selector;
mod document;
mod font_feature_values;
mod import;
mod keyframes;
mod layer;
mod media;
mod namespace;
mod page;
mod scope;
mod supports;

impl<'a> Parse<'a> for AtRule<'a> {
    fn parse(input: &mut Parser<'a>) -> PResult<Self> {
        let (at_keyword, at_keyword_span) = expect!(input, AtKeyword);

        let at_rule_name = at_keyword.ident.name();
        let (prelude, block, end) = if at_rule_name.eq_ignore_ascii_case("media") {
            // The typed grammar must account for the whole prelude; queries it
            // can't express (`@media all #{$m}`) are kept as raw tokens.
            let prelude = match input
                .try_parse_full_prelude(|p| MediaQueryList::parse(p).map(AtRulePrelude::Media))
            {
                Ok(prelude) => Some(prelude),
                Err(_) if matches!(peek!(input).token, Token::LBrace(..) | Token::Indent(..)) => {
                    None
                }
                // Only interpolation justifies the raw form — dart-sass
                // reparses such queries after resolving `#{...}`; plain
                // malformed logic (`@media a and b or c`) must keep erroring.
                Err(_) => {
                    let raw = if matches!(input.syntax, Syntax::Scss | Syntax::Sass) {
                        input.try_parse(|p| {
                            let raw = p.parse_raw_at_rule_prelude()?;
                            let has_interpolation = matches!(
                                &raw,
                                UnknownAtRulePrelude::TokenSeq(seq)
                                    if seq.tokens.iter().any(|t| matches!(
                                        t.token,
                                        Token::HashLBrace(..) | Token::StrTemplate(..)
                                    ))
                            );
                            if has_interpolation {
                                Ok(raw)
                            } else {
                                let span = peek!(p).span.clone();
                                Err(Error { kind: ErrorKind::TryParseError, span })
                            }
                        })
                    } else {
                        Err(Error {
                            kind: ErrorKind::TryParseError,
                            span: peek!(input).span.clone(),
                        })
                    };
                    match raw {
                        Ok(raw) => Some(AtRulePrelude::Unknown(arena_box!(input, raw))),
                        Err(_) => Some(AtRulePrelude::Media(input.parse()?)),
                    }
                }
            };
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("keyframes")
            || at_rule_name.eq_ignore_ascii_case("-webkit-keyframes")
            || at_rule_name.eq_ignore_ascii_case("-moz-keyframes")
            || at_rule_name.eq_ignore_ascii_case("-ms-keyframes")
            || at_rule_name.eq_ignore_ascii_case("-o-keyframes")
        {
            // A nameless `@keyframes {}` is invalid CSS, but Sass parses it
            // (the name may be produced elsewhere, e.g. a keyframes mixin
            // emitting vendor-prefixed blocks around `@content`).
            let prelude = match &peek!(input).token {
                Token::LBrace(..) => None,
                _ => {
                    // A typed name normally ends the prelude; real-world code
                    // also carries loose preludes (`@keyframes \$a`,
                    // `@-moz-keyframes name /* c */ line 429`) — keep those as
                    // raw tokens like an unknown at-rule's prelude.
                    let typed = input.try_parse_full_prelude(KeyframesName::parse);
                    match typed {
                        Ok(name) => Some(AtRulePrelude::Keyframes(name)),
                        Err(_) => input
                            .parse_unknown_at_rule_prelude()?
                            .map(|prelude| AtRulePrelude::Unknown(arena_box!(input, prelude))),
                    }
                }
            };
            let block = input
                .with_state(ParserState { in_keyframes_at_rule: true, ..input.state.clone() })
                .parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("import") {
            let (end, prelude) = match input.syntax {
                Syntax::Css => {
                    let prelude = input.parse::<ImportPrelude>()?;
                    (prelude.span.end, AtRulePrelude::Import(arena_box!(input, prelude)))
                }
                Syntax::Scss | Syntax::Sass => {
                    if let Ok(prelude) = input.try_parse(ImportPrelude::parse) {
                        (prelude.span.end, AtRulePrelude::Import(arena_box!(input, prelude)))
                    } else {
                        let prelude = input.parse::<SassImportPrelude>()?;
                        (prelude.span.end, AtRulePrelude::SassImport(prelude))
                    }
                }
                Syntax::Less => {
                    if let Ok(prelude) = input.try_parse(ImportPrelude::parse) {
                        (prelude.span.end, AtRulePrelude::Import(arena_box!(input, prelude)))
                    } else {
                        let prelude = input.parse::<LessImportPrelude>()?;
                        (prelude.span.end, AtRulePrelude::LessImport(arena_box!(input, prelude)))
                    }
                }
            };
            (Some(prelude), None, end)
        } else if at_rule_name.eq_ignore_ascii_case("charset") {
            // https://drafts.csswg.org/css2/#charset%E2%91%A0
            // Less may interpolate into it: `@charset "UTF-@{Eight}";`
            if input.syntax == Syntax::Less && matches!(peek!(input).token, Token::StrTemplate(..))
            {
                let prelude = input.parse::<InterpolableStr>()?;
                let end = prelude.span().end;
                (
                    Some(AtRulePrelude::Unknown(arena_box!(
                        input,
                        UnknownAtRulePrelude::ComponentValue(ComponentValue::InterpolableStr(
                            prelude
                        ))
                    ))),
                    None,
                    end,
                )
            } else {
                let prelude = input.parse::<Str>()?;
                let end = prelude.span.end;
                (Some(AtRulePrelude::Charset(prelude)), None, end)
            }
        } else if at_rule_name.eq_ignore_ascii_case("font-face") {
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (None, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("supports") {
            let prelude = Some(AtRulePrelude::Supports(input.parse()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("layer") {
            let prelude = match input.try_parse(LayerNames::parse) {
                Ok(names) => Some(AtRulePrelude::Layer(names)),
                // a Less variable may stand for the name: `@layer @layer-name {`
                Err(_)
                    if input.syntax == Syntax::Less
                        && matches!(peek!(input).token, Token::AtKeyword(..)) =>
                {
                    let raw = input.parse_raw_at_rule_prelude()?;
                    Some(AtRulePrelude::Unknown(arena_box!(input, raw)))
                }
                Err(_) => None,
            };
            let block = if matches!(peek!(input).token, Token::LBrace(..) | Token::Indent(..)) {
                Some(input.parse::<SimpleBlock>()?)
            } else {
                None
            };
            if let Some(block) = &block
                && matches!(&prelude, Some(AtRulePrelude::Layer(names)) if names.names.len() > 1)
            {
                input.recoverable_errors.push(Error {
                    kind: ErrorKind::UnexpectedSimpleBlock,
                    span: block.span.clone(),
                });
            }
            let end = block
                .as_ref()
                .map(|block| block.span.end)
                .or_else(|| prelude.as_ref().map(|prelude| prelude.span().end))
                .unwrap_or(at_keyword_span.end);
            (prelude, block, end)
        } else if at_rule_name.eq_ignore_ascii_case("container") {
            let prelude = Some(AtRulePrelude::Container(input.parse()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("page") {
            let prelude = input.try_parse(PageSelectorList::parse).map(AtRulePrelude::Page).ok();
            let block = input.try_parse(SimpleBlock::parse).ok();
            let end = block
                .as_ref()
                .map(|block| block.span.end)
                .or_else(|| prelude.as_ref().map(|prelude| prelude.span().end))
                .unwrap_or(at_keyword_span.end);
            (prelude, block, end)
        } else if at_rule_name.eq_ignore_ascii_case("namespace")
            && input.syntax == Syntax::Less
            && matches!(peek!(input).token, Token::AtKeyword(..))
        {
            // `@namespace @ns "http://...";` — a Less variable prefix
            let raw = input.parse_raw_at_rule_prelude()?;
            let end = raw.span().end;
            (Some(AtRulePrelude::Unknown(arena_box!(input, raw))), None, end)
        } else if at_rule_name.eq_ignore_ascii_case("namespace") {
            let namespace = input.parse::<NamespacePrelude>()?;
            let end = namespace.span.end;
            (Some(AtRulePrelude::Namespace(arena_box!(input, namespace))), None, end)
        } else if at_rule_name.eq_ignore_ascii_case("color-profile") {
            let prelude = Some(AtRulePrelude::ColorProfile(input.parse()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("font-feature-values") {
            let prelude = Some(AtRulePrelude::FontFeatureValues(input.parse()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("font-palette-values") {
            // https://drafts.csswg.org/css-fonts/Overview.bs
            let prelude = Some(AtRulePrelude::FontPaletteValues(input.parse_dashed_ident()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("counter-style") {
            let prelude = Some(AtRulePrelude::CounterStyle(input.parse_counter_style_prelude()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("custom-media") {
            let custom_media = input.parse::<CustomMedia>()?;
            let end = custom_media.span.end;
            (Some(AtRulePrelude::CustomMedia(arena_box!(input, custom_media))), None, end)
        } else if at_rule_name.eq_ignore_ascii_case("custom-selector") {
            let custom_selector_prelude = input.parse::<CustomSelectorPrelude>()?;
            let end = custom_selector_prelude.span.end;
            (
                Some(AtRulePrelude::CustomSelector(arena_box!(input, custom_selector_prelude))),
                None,
                end,
            )
        } else if at_rule_name.eq_ignore_ascii_case("position-try") {
            // https://drafts.csswg.org/css-anchor-position-1/#fallback-rule
            let prelude = Some(AtRulePrelude::PositionTry(input.parse_dashed_ident()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("nest") {
            // https://www.w3.org/TR/css-nesting-1/#at-nest
            let prelude = Some(AtRulePrelude::Nest(input.parse()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("property") {
            // https://drafts.css-houdini.org/css-properties-values-api/#at-property-rule
            let prelude = Some(AtRulePrelude::Property(input.parse_dashed_ident()?));
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("scope") {
            let prelude = if let Token::LParen(..) | Token::Ident(..) = peek!(input).token {
                Some(AtRulePrelude::Scope(arena_box!(input, input.parse()?)))
            } else {
                None
            };
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (prelude, Some(block), end)
        } else if at_rule_name.eq_ignore_ascii_case("document")
            || at_rule_name.eq_ignore_ascii_case("-moz-document")
        {
            let prelude = match input.try_parse(|p| p.parse().map(AtRulePrelude::Document)) {
                Ok(prelude) => prelude,
                // e.g. less.js's permissive `@-moz-document @fn("(x)")` — but
                // an empty prelude (`@document {}`) stays an error.
                Err(error) => {
                    if matches!(
                        peek!(input).token,
                        Token::LBrace(..) | Token::Indent(..) | Token::Semicolon(..)
                    ) {
                        return Err(error);
                    }
                    AtRulePrelude::Unknown(arena_box!(input, input.parse_raw_at_rule_prelude()?))
                }
            };
            // real-world code also writes a block-less `@document ...;`
            let block = if matches!(peek!(input).token, Token::LBrace(..) | Token::Indent(..)) {
                Some(input.parse::<SimpleBlock>()?)
            } else {
                None
            };
            let end = block.as_ref().map_or(prelude.span().end, |block| block.span.end);
            (Some(prelude), block, end)
        } else if at_rule_name.eq_ignore_ascii_case("stylistic")
            || at_rule_name.eq_ignore_ascii_case("historical-forms")
            || at_rule_name.eq_ignore_ascii_case("styleset")
            || at_rule_name.eq_ignore_ascii_case("character-variant")
            || at_rule_name.eq_ignore_ascii_case("swash")
            || at_rule_name.eq_ignore_ascii_case("ornaments")
            || at_rule_name.eq_ignore_ascii_case("annotation")
            || at_rule_name.eq_ignore_ascii_case("top-left-corner")
            || at_rule_name.eq_ignore_ascii_case("top-left")
            || at_rule_name.eq_ignore_ascii_case("top-center")
            || at_rule_name.eq_ignore_ascii_case("top-right")
            || at_rule_name.eq_ignore_ascii_case("top-right-corner")
            || at_rule_name.eq_ignore_ascii_case("bottom-left-corner")
            || at_rule_name.eq_ignore_ascii_case("bottom-left")
            || at_rule_name.eq_ignore_ascii_case("bottom-center")
            || at_rule_name.eq_ignore_ascii_case("bottom-right")
            || at_rule_name.eq_ignore_ascii_case("bottom-right-corner")
            || at_rule_name.eq_ignore_ascii_case("left-top")
            || at_rule_name.eq_ignore_ascii_case("left-middle")
            || at_rule_name.eq_ignore_ascii_case("left-bottom")
            || at_rule_name.eq_ignore_ascii_case("right-top")
            || at_rule_name.eq_ignore_ascii_case("right-middle")
            || at_rule_name.eq_ignore_ascii_case("right-bottom")
            || at_rule_name.eq_ignore_ascii_case("viewport")
            || at_rule_name.eq_ignore_ascii_case("try")
            || at_rule_name.eq_ignore_ascii_case("starting-style")
        {
            let block = input.parse::<SimpleBlock>()?;
            let end = block.span.end;
            (None, Some(block), end)
        } else if at_rule_name == "plugin" && input.syntax == Syntax::Less {
            let prelude = input.parse::<LessPlugin>()?;
            let end = prelude.span.end;
            (Some(AtRulePrelude::LessPlugin(arena_box!(input, prelude))), None, end)
        } else if at_rule_name.eq_ignore_ascii_case("function")
            && matches!(&peek!(input).token, Token::Ident(ident) if ident.raw.starts_with("--"))
        {
            // A CSS custom function (css-mixins spec): `@function --name(params)
            // returns <type> { declarations }`. dart-sass parses this as plain
            // CSS in every syntax (the `--` name marks it), so the prelude is
            // kept as raw tokens and the body takes declarations with raw
            // values.
            let prelude = input.parse_raw_at_rule_prelude()?;
            let block = input
                .with_state(ParserState { in_css_function_body: true, ..input.state.clone() })
                .parse::<SimpleBlock>()?;
            let end = block.span.end;
            (Some(AtRulePrelude::Unknown(arena_box!(input, prelude))), Some(block), end)
        } else if matches!(input.syntax, Syntax::Scss | Syntax::Sass) {
            use super::state::{
                SASS_CTX_ALLOW_DIV, SASS_CTX_ALLOW_KEYFRAME_BLOCK, SASS_CTX_IN_FUNCTION,
            };
            match &*at_rule_name {
                "each" => {
                    let prelude = input.parse()?;
                    let block = input.parse::<SimpleBlock>()?;
                    let end = block.span.end;
                    (Some(AtRulePrelude::SassEach(arena_box!(input, prelude))), Some(block), end)
                }
                "while" => {
                    input.eat_sass_line_continuation()?;
                    let prelude = input.parse()?;
                    let block = input.parse::<SimpleBlock>()?;
                    let end = block.span.end;
                    (Some(AtRulePrelude::SassExpr(arena_box!(input, prelude))), Some(block), end)
                }
                "for" => {
                    let prelude = input.parse()?;
                    let block = input.parse::<SimpleBlock>()?;
                    let end = block.span.end;
                    (Some(AtRulePrelude::SassFor(arena_box!(input, prelude))), Some(block), end)
                }
                "mixin" => {
                    let prelude = input.parse()?;
                    let block = input
                        .with_state(ParserState {
                            sass_ctx: input.state.sass_ctx | SASS_CTX_ALLOW_KEYFRAME_BLOCK,
                            ..input.state.clone()
                        })
                        .parse::<SimpleBlock>()?;
                    let end = block.span.end;
                    (Some(AtRulePrelude::SassMixin(arena_box!(input, prelude))), Some(block), end)
                }
                "include" => {
                    let prelude = input.parse::<SassInclude>()?;
                    let block =
                        if matches!(peek!(input).token, Token::LBrace(..) | Token::Indent(..)) {
                            Some(
                                input
                                    .with_state(ParserState {
                                        sass_ctx: input.state.sass_ctx
                                            | SASS_CTX_ALLOW_KEYFRAME_BLOCK,
                                        ..input.state.clone()
                                    })
                                    .parse::<SimpleBlock>()?,
                            )
                        } else {
                            None
                        };
                    let end =
                        block.as_ref().map(|block| block.span.end).unwrap_or(prelude.span.end);
                    (Some(AtRulePrelude::SassInclude(arena_box!(input, prelude))), block, end)
                }
                "content" => {
                    if matches!(peek!(input).token, Token::LParen(..)) {
                        let prelude = input.parse::<SassContent>()?;
                        let end = prelude.span.end;
                        (Some(AtRulePrelude::SassContent(prelude)), None, end)
                    } else {
                        (None, None, input.tokenizer.current_offset())
                    }
                }
                "use" => {
                    let prelude = input.parse::<SassUse>()?;
                    let end = prelude.span.end;
                    (Some(AtRulePrelude::SassUse(arena_box!(input, prelude))), None, end)
                }
                "function" => {
                    let prelude = input.parse::<SassFunction>()?;
                    let block = input
                        .with_state(ParserState {
                            sass_ctx: input.state.sass_ctx | SASS_CTX_IN_FUNCTION,
                            ..input.state.clone()
                        })
                        .parse::<SimpleBlock>()?;
                    let end = block.span.end;
                    (
                        Some(AtRulePrelude::SassFunction(arena_box!(input, prelude))),
                        Some(block),
                        end,
                    )
                }
                "return" => {
                    input.eat_sass_line_continuation()?;
                    let expr = input
                        .with_state(ParserState {
                            sass_ctx: input.state.sass_ctx | SASS_CTX_ALLOW_DIV,
                            ..input.state.clone()
                        })
                        .parse_maybe_sass_list(/* allow_comma */ true)?;
                    let end = expr.span().end;
                    if input.state.sass_ctx & SASS_CTX_IN_FUNCTION == 0 {
                        input.recoverable_errors.push(Error {
                            kind: ErrorKind::ReturnOutsideFunction,
                            span: Span { start: at_keyword_span.start, end },
                        });
                    }
                    (Some(AtRulePrelude::SassExpr(arena_box!(input, expr))), None, end)
                }
                "extend" => {
                    let prelude = input.parse::<SassExtend>()?;
                    let end = prelude.span.end;
                    (Some(AtRulePrelude::SassExtend(arena_box!(input, prelude))), None, end)
                }
                "warn" | "error" | "debug" => {
                    input.eat_sass_line_continuation()?;
                    let expr = input.parse_maybe_sass_list(/* allow_comma */ true)?;
                    let end = expr.span().end;
                    (Some(AtRulePrelude::SassExpr(arena_box!(input, expr))), None, end)
                }
                "forward" => {
                    let prelude = input.parse::<SassForward>()?;
                    let end = prelude.span.end;
                    (Some(AtRulePrelude::SassForward(arena_box!(input, prelude))), None, end)
                }
                "at-root" => {
                    let prelude = if !matches!(
                        peek!(input).token,
                        Token::LBrace(..)
                            | Token::Indent(..)
                            | Token::Linebreak(..)
                            | Token::Dedent(..)
                            | Token::Eof(..)
                    ) {
                        Some(AtRulePrelude::SassAtRoot(input.parse()?))
                    } else {
                        None
                    };
                    // `@at-root` escapes surrounding contexts, including a
                    // `@keyframes` body — its block holds normal rules.
                    let block = input
                        .with_state(ParserState {
                            in_keyframes_at_rule: false,
                            ..input.state.clone()
                        })
                        .parse::<SimpleBlock>()?;
                    let end = block.span.end;
                    (prelude, Some(block), end)
                }
                _ => {
                    let (prelude, block, end) = input.parse_unknown_at_rule()?;
                    (
                        prelude.map(|prelude| AtRulePrelude::Unknown(arena_box!(input, prelude))),
                        block,
                        end.unwrap_or(at_keyword_span.end),
                    )
                }
            }
        } else {
            let (prelude, block, end) = input.parse_unknown_at_rule()?;
            (
                prelude.map(|prelude| AtRulePrelude::Unknown(arena_box!(input, prelude))),
                block,
                end.unwrap_or(at_keyword_span.end),
            )
        };

        let span = Span { start: at_keyword_span.start, end };
        Ok(AtRule {
            name: input.ident(
                at_keyword.ident,
                Span { start: at_keyword_span.start + 1, end: at_keyword_span.end },
            ),
            prelude,
            block,
            span,
        })
    }
}

impl<'a> Parser<'a> {
    /// `try_parse` a typed at-rule prelude that must account for everything
    /// up to the end of the prelude (the block's opener or the statement
    /// boundary) — otherwise the parse is rolled back so the caller can fall
    /// back to a raw form.
    fn try_parse_full_prelude<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        self.try_parse(|p| {
            let value = f(p)?;
            match &peek!(p).token {
                Token::LBrace(..)
                | Token::Indent(..)
                | Token::Semicolon(..)
                | Token::Dedent(..)
                | Token::Linebreak(..)
                | Token::Eof(..) => Ok(value),
                _ => {
                    let span = peek!(p).span.clone();
                    Err(Error { kind: ErrorKind::TryParseError, span })
                }
            }
        })
    }

    /// A raw at-rule prelude: everything up to the body's `{` (or the end of
    /// the statement), balancing pairs so interpolations and parens pass
    /// through — CSS custom function preludes (`--name(--arg) returns <type>`)
    /// and media queries the typed grammar can't express.
    fn parse_raw_at_rule_prelude(&mut self) -> PResult<UnknownAtRulePrelude<'a>> {
        let start = self.tokenizer.current_offset();
        let mut tokens = self.vec();
        let mut pairs: Vec<crate::util::PairedToken> = Vec::new();
        loop {
            match &peek!(self).token {
                Token::Semicolon(..)
                | Token::Dedent(..)
                | Token::Linebreak(..)
                | Token::Indent(..)
                | Token::Eof(..) => break,
                Token::LBrace(..) if pairs.is_empty() => break,
                // Interpolated strings must be consumed structurally — the
                // tokenizer resumes the string after each `#{...}` — but
                // their pieces are still plain tokens.
                Token::StrTemplate(..) => {
                    self.consume_str_template_tokens_into(&mut tokens)?;
                    continue;
                }
                token => {
                    if !crate::util::track_paired_token(token, &mut pairs) {
                        break;
                    }
                }
            }
            tokens.push(bump!(self));
        }
        let span = Span {
            start: tokens.first().map_or(start, |token| token.span.start),
            end: tokens.last().map_or(start, |token| token.span.end),
        };
        Ok(UnknownAtRulePrelude::TokenSeq(TokenSeq { tokens, span }))
    }

    pub(super) fn parse_unknown_at_rule(
        &mut self,
    ) -> PResult<(Option<UnknownAtRulePrelude<'a>>, Option<SimpleBlock<'a>>, Option<usize>)> {
        let prelude = self.parse_unknown_at_rule_prelude()?;
        let block = match &peek!(self).token {
            // An unknown at-rule's children parse generically; in Sass that
            // includes keyframe-style `10% { ... }` blocks
            // (`@keyfr#{"ames"} a {...}`). less.js keeps them an error.
            Token::LBrace(..) | Token::Indent(..) => {
                let sass_ctx = if matches!(self.syntax, Syntax::Scss | Syntax::Sass) {
                    self.state.sass_ctx | super::state::SASS_CTX_ALLOW_KEYFRAME_BLOCK
                } else {
                    self.state.sass_ctx
                };
                Some(
                    self.with_state(ParserState { sass_ctx, ..self.state.clone() })
                        .parse::<SimpleBlock>()?,
                )
            }
            _ => None,
        };
        let end = block
            .as_ref()
            .map(|block| block.span.end)
            .or_else(|| prelude.as_ref().map(|prelude| prelude.span().end));
        Ok((prelude, block, end))
    }

    fn parse_unknown_at_rule_prelude(&mut self) -> PResult<Option<UnknownAtRulePrelude<'a>>> {
        if let Ok(prelude) = self.try_parse(|parser| {
            let mut tokens = parser.vec();
            loop {
                match &peek!(parser).token {
                    Token::LBrace(..)
                    | Token::RBrace(..)
                    | Token::Semicolon(..)
                    | Token::Indent(..)
                    | Token::Dedent(..)
                    | Token::Linebreak(..)
                    | Token::Eof(..) => break,
                    Token::StrTemplate(..) | Token::HashLBrace(..) => {
                        return Err(Error {
                            kind: ErrorKind::TryParseError,
                            span: bump!(parser).span,
                        });
                    }
                    _ => tokens.push(bump!(parser)),
                }
            }
            if let Some((first, last)) = tokens.first().zip(tokens.last()) {
                let span = Span { start: first.span().start, end: last.span().end };
                Ok(Some(UnknownAtRulePrelude::TokenSeq(TokenSeq { tokens, span })))
            } else {
                Ok(None)
            }
        }) {
            return Ok(prelude);
        }

        Ok(Some(UnknownAtRulePrelude::ComponentValue(match self.syntax {
            Syntax::Css => self.parse()?,
            Syntax::Scss | Syntax::Sass => {
                self.parse_maybe_sass_list(/* allow_comma */ true)?
            }
            Syntax::Less => self.parse_maybe_less_list(/* allow_comma */ true)?,
        })))
    }
}

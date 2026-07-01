use super::Parser;
use std::{
    mem,
    ops::{Deref, DerefMut},
};

#[derive(Clone, Debug, Default)]
pub(super) struct ParserState {
    pub(super) qualified_rule_ctx: Option<QualifiedRuleContext>,
    pub(super) sass_ctx: u8,
    pub(super) less_ctx: u8,
    pub(super) in_keyframes_at_rule: bool,
    /// Enabled only while parsing a declaration that is a statement in a style-rule
    /// block, so the IE `*color` hack does not leak into feature queries
    /// (`@supports`, `@container style()`, `@import supports()`).
    pub(super) allow_ie_star_hack: bool,
}

#[derive(Clone, Debug)]
pub(super) enum QualifiedRuleContext {
    Selector,
    DeclarationName,
    DeclarationValue,
}

pub(super) const SASS_CTX_IN_FUNCTION: u8 = 1;
pub(super) const SASS_CTX_ALLOW_DIV: u8 = 2;
pub(super) const SASS_CTX_ALLOW_KEYFRAME_BLOCK: u8 = 4;
pub(super) const SASS_CTX_IN_PARENS: u8 = 8;

pub(super) const LESS_CTX_ALLOW_DIV: u8 = 1;
pub(super) const LESS_CTX_ALLOW_KEYFRAME_BLOCK: u8 = 2;

impl<'a> Parser<'a> {
    pub(super) fn with_state(&mut self, state: ParserState) -> WithState<'a, '_> {
        let original_state = mem::replace(&mut self.state, state);
        WithState { parser: self, original_state }
    }
}

pub(super) struct WithState<'a, 'p> {
    parser: &'p mut Parser<'a>,
    original_state: ParserState,
}

impl<'a, 'p> Deref for WithState<'a, 'p> {
    type Target = Parser<'a>;

    fn deref(&self) -> &Self::Target {
        self.parser
    }
}

impl<'a, 'p> DerefMut for WithState<'a, 'p> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.parser
    }
}

impl<'a, 'p> Drop for WithState<'a, 'p> {
    fn drop(&mut self) {
        mem::swap(&mut self.parser.state, &mut self.original_state);
    }
}

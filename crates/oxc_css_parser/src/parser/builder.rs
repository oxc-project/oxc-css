use super::Parser;
use crate::{ParserOptions, Syntax, tokenizer::Tokenizer};
use oxc_allocator::Allocator;

/// Parser builder is for building a parser while allowing us
/// to control advanced behaviors.
///
/// Unlike [`Parser`], syntax isn't required when creating a parser builder,
/// and the default syntax will be CSS. If you need to parse with another syntax,
/// use the [`syntax`](ParserBuilder::syntax) to modify it.
pub struct ParserBuilder<'a> {
    allocator: &'a Allocator,
    source: &'a str,
    syntax: Syntax,
    options: Option<ParserOptions>,
    collect_comments: bool,
}

impl<'a> ParserBuilder<'a> {
    /// Create a parser builder from given source code.
    pub fn new(allocator: &'a Allocator, source: &'a str) -> Self {
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        ParserBuilder {
            allocator,
            source,
            options: None,
            syntax: Syntax::default(),
            collect_comments: false,
        }
    }

    /// Specify the syntax for parsing.
    pub fn syntax(mut self, syntax: Syntax) -> Self {
        self.syntax = syntax;
        self
    }

    /// Customize parser options.
    pub fn options(mut self, options: ParserOptions) -> Self {
        self.options = Some(options);
        self
    }

    /// Collect comments.
    pub fn comments(mut self) -> Self {
        self.collect_comments = true;
        self
    }

    /// Disable collecting comments.
    ///
    /// Collecting comments is disabled by default,
    /// so you don't need to use this if you never call the [`comments`](ParserBuilder::comments) method.
    pub fn ignore_comments(mut self) -> Self {
        self.collect_comments = false;
        self
    }

    /// Build a parser.
    pub fn build(self) -> Parser<'a> {
        let options = self.options.unwrap_or_default();
        // Backtick is not valid CSS/SCSS/Sass; the placeholder lexer path only
        // makes sense for SCSS (in Less, backtick is the inline-JS delimiter).
        debug_assert!(
            options.template_placeholder.is_none() || self.syntax == Syntax::Scss,
            "template_placeholder requires Syntax::Scss (backtick is Less's inline-JS delimiter)",
        );
        Parser {
            allocator: self.allocator,
            source: self.source,
            syntax: self.syntax,
            options,
            tokenizer: Tokenizer::new(
                self.allocator,
                self.source,
                self.syntax,
                options.template_placeholder,
                self.collect_comments,
            ),
            state: Default::default(),
            recoverable_errors: vec![],
            cached_token: None,
        }
    }
}

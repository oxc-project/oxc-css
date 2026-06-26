//! oxc-css-parser is a parser that can parse CSS, SCSS, Sass (indented syntax) and Less.
//!
//! ## Basic Usage
//!
//! This crate provides a simple API to get started.
//!
//! First, create a parser, give it the source code and specify the syntax,
//! then call the [`parse`](Parser::parse) method:
//!
//! ```rust
//! use oxc_css_parser::{Allocator, Parser, Syntax, ast::Stylesheet};
//!
//! let allocator = Allocator::default();
//! let mut parser = Parser::new(&allocator, "a {}", Syntax::Css); // syntax can also be `Scss`, `Sass` or `Less`
//! let result = parser.parse::<Stylesheet>();
//! match result {
//!     Ok(ast) => {
//!         // parsed successfully
//!         println!("{:#?}", ast);
//!     }
//!     Err(error) => {
//!         // it failed, error message and position can be accessed via `error`
//!         println!("{:#?}", error);
//!     }
//! }
//! ```
//!
//! ## Advanced Usage
//!
//! ### Creating Parser with Builder
//!
//! If you need to control parser with additional features, you can use [`ParserBuilder`].
//!
//! For example, to collect comments:
//!
//! ```rust
//! use oxc_css_parser::{Allocator, ParserBuilder, ast::Stylesheet};
//!
//! let allocator = Allocator::default();
//! let builder = ParserBuilder::new(&allocator, "/* comment */ a {}").comments();
//! let mut parser = builder.build();
//! parser.parse::<Stylesheet>().unwrap();
//! let comments = parser.comments();
//! ```
//!
//! By default, syntax is CSS when using parser builder. You can customize it:
//!
//! ```rust
//! use oxc_css_parser::{Allocator, ParserBuilder, Syntax};
//!
//! let allocator = Allocator::default();
//! let builder = ParserBuilder::new(&allocator, "a {}").syntax(Syntax::Scss);
//! ```
//!
//! ### Parser Options
//!
//! #### `try_parsing_value_in_custom_property`
//!
//! By default, value of custom property whose name starts with `--` will be parsed as tokens.
//! If you want to parse it as normal declaration value, you can enable this option.
//! Even though this option is enabled,
//! parser will fallback to parse as tokens if there're syntax errors.
//!
//! ```rust
//! use oxc_css_parser::{Allocator, ParserBuilder, ParserOptions, ast::*};
//!
//! let allocator = Allocator::default();
//! let options = ParserOptions {
//!     try_parsing_value_in_custom_property: true,
//!     ..Default::default()
//! };
//! let builder = ParserBuilder::new(&allocator, "--foo: calc(var(--bar) + 1px)").options(options);
//! let mut parser = builder.build();
//!
//! let declaration = parser.parse::<Declaration>().unwrap();
//! assert!(matches!(declaration.value[0], ComponentValue::Function(..)));
//! ```
//!
//! #### `tolerate_semicolon_in_sass`
//!
//! For Sass (not SCSS), semicolons for every statements are syntax errors.
//! By default, parser will raise a syntax error and return `Err` when
//! encountered this.
//! Enabling this option can turn such syntax errors into recoverable errors,
//! so they won't prevent parsing the rest of code.
//!
//! ```rust
//! use oxc_css_parser::{Allocator, ParserBuilder, ParserOptions, Syntax, ast::*};
//!
//! let allocator = Allocator::default();
//! let options = ParserOptions {
//!     tolerate_semicolon_in_sass: true,
//!     ..Default::default()
//! };
//! let builder = ParserBuilder::new(&allocator, "
//! button
//!   width: 12px;
//!   height: 12px;
//! ").syntax(Syntax::Sass).options(options);
//! let mut parser = builder.build();
//!
//! assert!(parser.parse::<Stylesheet>().is_ok());
//! assert_eq!(parser.recoverable_errors().len(), 2);
//! ```
//!
//! #### `template_placeholder`
//!
//! By default, a backtick is a syntax error outside Less. Setting this option
//! makes the parser recognize a backtick-delimited token of the shape
//! `` `<prefix><decimal index>` `` as an atomic
//! [`Placeholder`](crate::ast::Placeholder) node (in value, selector, and
//! statement positions) carrying the parsed index. The token terminates at the
//! closing backtick, so a following identifier re-lexes separately. This is
//! designed for downstream formatters that substitute template interpolations
//! (e.g. CSS-in-JS `${expr}`) with such placeholders before parsing. It MUST be
//! used with [`Syntax::Scss`] (backtick is Less's inline-JS delimiter).
//!
//! ```rust
//! use oxc_css_parser::{Allocator, ParserBuilder, ParserOptions, Syntax, TemplatePlaceholder, ast::*};
//!
//! let allocator = Allocator::default();
//! let options = ParserOptions {
//!     template_placeholder: Some(TemplatePlaceholder {
//!         prefix: "PLACEHOLDER-",
//!     }),
//!     ..Default::default()
//! };
//! let builder = ParserBuilder::new(&allocator, "a { width: `PLACEHOLDER-0`; }")
//!     .syntax(Syntax::Scss)
//!     .options(options);
//! let mut parser = builder.build();
//!
//! assert!(parser.parse::<Stylesheet>().is_ok());
//! ```
//!
//! ### Parse Partial Structure
//!
//! Sometimes you don't want to parse a full stylesheet.
//! Say you only need to parse a qualified rule or even a single declaration.
//! All you need to do is to update the generics of the [`parse`](Parser::parse) method.
//!
//! ```rust
//! use oxc_css_parser::{Allocator, Parser, Syntax, ast::QualifiedRule};
//!
//! let allocator = Allocator::default();
//! let mut parser = Parser::new(&allocator, "a {}", Syntax::Css);
//! parser.parse::<QualifiedRule>();
//! ```
//!
//! and
//!
//! ```rust
//! use oxc_css_parser::{Allocator, Parser, Syntax, ast::Declaration};
//!
//! let allocator = Allocator::default();
//! let mut parser = Parser::new(&allocator, "color: green", Syntax::Css);
//! parser.parse::<Declaration>();
//! ```
//!
//! Not all AST nodes support the usage above;
//! technically, those nodes that implement [`Parse`] trait are supported.
//!
//! ### Retrieve Recoverable Errors
//!
//! There may be some recoverable errors which doesn't affect on producing AST.
//! To retrieve those errors, use [`recoverable_errors`](Parser::recoverable_errors).
//!
//! ```rust
//! use oxc_css_parser::{Allocator, Parser, Syntax, ast::Stylesheet};
//!
//! let allocator = Allocator::default();
//! let mut parser = Parser::new(&allocator, "@keyframes kf { invalid {} }", Syntax::Css);
//! let result = parser.parse::<Stylesheet>();
//! assert!(result.is_ok());
//! println!("{:?}", parser.recoverable_errors());
//! ```
//!
//! ## Serialization
//!
//! Produced AST can be serialized by Serde, but this feature is disabled by default.
//! You need to enable feature `serialize` manually:
//!
//! ```toml
//! oxc-css-parser = { version = "*", features = ["serialize"] }
//! ```
//!
//! Then you can pass AST to Serde.
//!
//! Note that oxc-css-parser only supports serialization. Deserialization isn't supported.

pub use config::{ParserOptions, Syntax, TemplatePlaceholder};
pub use oxc_allocator::Allocator;
pub use parser::{Parse, Parser, ParserBuilder};
pub use pos::{Span, Spanned};
pub use span_ignored_eq::SpanIgnoredEq;
pub use tokenizer::token;

pub mod ast;
mod config;
pub mod error;
mod parser;
pub mod pos;
mod span_ignored_eq;
mod tokenizer;
mod util;

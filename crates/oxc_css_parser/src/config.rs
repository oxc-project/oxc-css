#[cfg(feature = "config_serde")]
use serde::{Deserialize, Serialize};

/// Supported syntax.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "config_serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "config_serde", serde(rename_all = "camelCase"))]
pub enum Syntax {
    #[default]
    Css,
    Scss,
    /// Indented Sass Syntax
    Sass,
    Less,
}

/// Configuration for a backtick-delimited template placeholder.
///
/// A placeholder has the shape `` `<prefix><decimal index>` `` (e.g. with
/// `prefix = "PLACEHOLDER-"`: `` `PLACEHOLDER-0` ``). The opening and closing
/// backticks are fixed; only `prefix` is supplied by the downstream consumer, so
/// oxc-css-parser stays agnostic to its content and only uses it to recognize and index
/// the token.
///
/// Backtick is not valid CSS/SCSS/Sass syntax (it is only Less's inline-JS
/// delimiter), so this is intended for SCSS parsing only; see
/// [`ParserOptions::template_placeholder`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplatePlaceholder {
    /// Text after the opening backtick that marks the start of a placeholder,
    /// before its decimal index (e.g. `"PLACEHOLDER-"`).
    pub prefix: &'static str,
}

/// Parser options for customizing parser behaviors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "config_serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "config_serde", serde(rename_all = "camelCase"))]
pub struct ParserOptions {
    /// Enabling this will make parser attempt to parse
    /// custom property value as normal declaration value instead of tokens.
    /// It will fallback to parse as tokens if there're syntax errors
    /// when parsing as values.
    pub try_parsing_value_in_custom_property: bool,

    /// If enabled, [`Syntax::Css`] accepts the `$variable` syntax handled by the
    /// [`postcss-simple-vars`](https://github.com/postcss/postcss-simple-vars) plugin:
    /// `$var: value;` declarations, `$var` references in property values,
    /// and `$var` references inside `@media` (and similar) at-rule preludes.
    ///
    /// The resulting AST uses dedicated [`PostcssSimpleVar`](crate::ast::PostcssSimpleVar)
    /// and [`PostcssSimpleVarDeclaration`](crate::ast::PostcssSimpleVarDeclaration)
    /// nodes, separate from SCSS's [`SassVariable`](crate::ast::SassVariable) family.
    ///
    /// NOTE: Interpolation (`$(var)`), selector references (`.$prefix`),
    /// and comment substitutions (`<<$(var)>>`) are not yet covered.
    ///
    /// Ignored for [`Syntax::Scss`], [`Syntax::Sass`], and [`Syntax::Less`]
    /// (those dialects already accept `$variable` natively).
    pub allow_postcss_simple_vars: bool,

    /// If set, a backtick-delimited token of the shape `` `<prefix><decimal index>` ``
    /// (see [`TemplatePlaceholder`]) is tokenized as an atomic
    /// [`Token::Placeholder`](crate::token::Token) carrying the parsed index.
    /// The token terminates at the closing backtick, so an immediately following
    /// identifier (e.g. `` `PLACEHOLDER-0`px ``) is lexed as a separate suffix.
    ///
    /// This is designed for downstream formatters that substitute template
    /// interpolations (e.g. CSS-in-JS `${expr}`) with such placeholders
    /// before parsing, then re-substitute them in the output.
    ///
    /// Backtick is not valid CSS/SCSS/Sass syntax, so this MUST be used with
    /// [`Syntax::Scss`] (the parser builder asserts this); in Less, backtick is
    /// the inline-JS delimiter and would conflict.
    ///
    /// Not serialized: the affix is a `&'static str` supplied programmatically,
    /// not loadable from a config file.
    #[cfg_attr(feature = "config_serde", serde(skip))]
    pub template_placeholder: Option<TemplatePlaceholder>,
}

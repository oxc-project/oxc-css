use oxc_css_parser::{Allocator, ParserBuilder, ParserOptions, Syntax, ast::*};

fn opts() -> ParserOptions {
    ParserOptions { allow_postcss_simple_vars: true, ..Default::default() }
}

fn parse_css(code: &'static str, options: Option<ParserOptions>) -> Stylesheet<'static> {
    let allocator = Box::leak(Box::new(Allocator::default()));
    let mut builder = ParserBuilder::new(allocator, code).syntax(Syntax::Css);
    if let Some(options) = options {
        builder = builder.options(options);
    }
    builder.build().parse::<Stylesheet>().unwrap()
}

fn parse_css_err(code: &'static str, options: Option<ParserOptions>) {
    let allocator = Allocator::default();
    let mut builder = ParserBuilder::new(&allocator, code).syntax(Syntax::Css);
    if let Some(options) = options {
        builder = builder.options(options);
    }
    assert!(builder.build().parse::<Stylesheet>().is_err());
}

#[test]
fn default_options_reject_dollar_variable_declaration() {
    parse_css_err("$primary: red;", None);
}

// Without the flag there is no structured node, but the declaration still
// parses: `$` is a valid CSS `<delim-token>`, so the value falls back to raw
// preserved tokens (contrast with the structured `PostcssSimpleVar` below).
#[test]
fn default_options_keep_dollar_variable_reference_as_raw_tokens() {
    let ss = parse_css(".a { color: $primary; }", None);
    let Statement::QualifiedRule(rule) = &ss.statements[0] else {
        panic!("expected qualified rule");
    };
    let Statement::Declaration(decl) = &rule.block.statements[0] else {
        panic!("expected declaration");
    };
    assert!(matches!(decl.value[0], ComponentValue::TokenWithSpan(_)));
}

// `@media (max-width: $var)` parses even without the flag because the parser
// falls back to `<general-enclosed>` (W3C media query forward-compat). That
// path keeps the prelude as a raw `TokenSeq`, so formatter output is verbatim.
#[test]
fn default_options_treat_dollar_variable_in_media_query_as_general_enclosed() {
    let _ = parse_css("@media (max-width: $bp) { .a { color: red; } }", None);
}

#[test]
fn opt_in_accepts_dollar_variable_declaration() {
    let ss = parse_css("$primary: red;", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::PostcssSimpleVarDeclaration(_)));
}

#[test]
fn opt_in_accepts_dollar_variable_reference_in_value() {
    let ss = parse_css(".a { color: $primary; }", Some(opts()));
    let Statement::QualifiedRule(rule) = &ss.statements[0] else {
        panic!("expected qualified rule");
    };
    let Statement::Declaration(decl) = &rule.block.statements[0] else {
        panic!("expected declaration");
    };
    assert!(matches!(decl.value[0], ComponentValue::PostcssSimpleVar(_)));
}

#[test]
fn opt_in_accepts_dollar_variable_in_media_query() {
    let _ = parse_css("@media (max-width: $bp) { .a { color: red; } }", Some(opts()));
}

#[test]
fn opt_in_preserves_important_annotation_in_value() {
    let ss = parse_css("$primary: red !important;", Some(opts()));
    let Statement::PostcssSimpleVarDeclaration(decl) = &ss.statements[0] else {
        panic!("expected dollar variable declaration");
    };
    assert!(matches!(decl.value.last(), Some(ComponentValue::ImportantAnnotation(_))));
}

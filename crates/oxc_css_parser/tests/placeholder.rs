use oxc_css_parser::{
    Allocator, ParserBuilder, ParserOptions, Syntax, TemplatePlaceholder, ast::*,
};

fn opts() -> ParserOptions {
    ParserOptions {
        template_placeholder: Some(TemplatePlaceholder { prefix: "PLACEHOLDER-" }),
        ..Default::default()
    }
}

fn parse(code: &'static str, options: Option<ParserOptions>) -> Stylesheet<'static> {
    // Backtick placeholders require SCSS (the builder asserts this); without the
    // option, parse as plain CSS so the backtick stays an ordinary error.
    let allocator = Box::leak(Box::new(Allocator::default()));
    let syntax = if options.is_some() { Syntax::Scss } else { Syntax::Css };
    let mut builder = ParserBuilder::new(allocator, code).syntax(syntax);
    if let Some(options) = options {
        builder = builder.options(options);
    }
    builder.build().parse::<Stylesheet>().unwrap()
}

#[test]
fn default_options_do_not_parse_placeholders() {
    // Without the option set, a backtick is not special (it's an ordinary syntax
    // error outside Less); the tokenizer never emits `Token::Placeholder`.
    let allocator = Allocator::default();
    let builder = ParserBuilder::new(&allocator, "`PLACEHOLDER-0`;").syntax(Syntax::Css);
    assert!(builder.build().parse::<Stylesheet>().is_err());
}

#[test]
fn real_at_rules_unaffected_by_option() {
    // An at-keyword is still a normal at-rule; the option only affects backticks.
    let ss = parse("@media x{a{color:red}}", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::AtRule(_)));
}

#[test]
fn statement_position_placeholder() {
    let ss = parse("`PLACEHOLDER-3`;", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::Placeholder(Placeholder { index: 3, .. })));
}

#[test]
fn value_position_placeholder_glues_following_ident_as_suffix() {
    // `` `PLACEHOLDER-0`px `` => a single Placeholder(0) carrying suffix `"px"`,
    // mirroring `#{$x}px` being one identifier (not Placeholder + ident `px`).
    let ss = parse("a{width:`PLACEHOLDER-0`px}", Some(opts()));
    let Statement::QualifiedRule(qr) = &ss.statements[0] else {
        panic!("expected qualified rule");
    };
    let Statement::Declaration(decl) = &qr.block.statements[0] else {
        panic!("expected declaration");
    };
    assert_eq!(decl.value.len(), 1);
    assert!(matches!(
        &decl.value[0],
        ComponentValue::Placeholder(Placeholder { index: 0, suffix: "px", .. })
    ));
}

#[test]
fn bare_placeholder_has_empty_suffix() {
    // A whitespace/delimiter right after the closing backtick => no glued suffix.
    let ss = parse("`PLACEHOLDER-3`;", Some(opts()));
    assert!(matches!(
        ss.statements[0],
        Statement::Placeholder(Placeholder { index: 3, suffix: "", .. })
    ));
}

#[test]
fn plain_class_selector_not_broken_by_option() {
    // Regression: enabling the option must not panic on ordinary class
    // selectors. The class name detection must not call `peek!` (which caches
    // a token and trips the empty-cache assertion in the no-ws ident path).
    let ss = parse(".foo.bar{color:red}", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::QualifiedRule(_)));
}

#[test]
fn placeholder_as_rule_selector() {
    // Regression: a placeholder substituting a selector (CSS-in-JS
    // `${Component} { ... }`) must parse as a qualified rule, not be swallowed
    // as a standalone placeholder statement.
    let ss = parse("`PLACEHOLDER-0`{color:red}", Some(opts()));
    let Statement::QualifiedRule(qr) = &ss.statements[0] else {
        panic!("expected qualified rule");
    };
    assert!(matches!(qr.block.statements[0], Statement::Declaration(_)));
}

#[test]
fn placeholder_class_selector_name() {
    // A placeholder immediately following a `.` is the class name.
    let ss = parse(".`PLACEHOLDER-2`{color:red}", Some(opts()));
    let Statement::QualifiedRule(qr) = &ss.statements[0] else {
        panic!("expected qualified rule");
    };
    let ComplexSelector { children, .. } = &qr.selector.selectors[0];
    let ComplexSelectorChild::CompoundSelector(compound) = &children[0] else {
        panic!("expected compound selector");
    };
    assert!(matches!(
        &compound.children[0],
        SimpleSelector::Class(ClassSelector {
            name: InterpolableIdent::Placeholder(Placeholder { index: 2, .. }),
            ..
        })
    ));
}

#[test]
fn placeholder_id_selector_name() {
    // A placeholder immediately following a `#` is the id name (`#${id}`).
    let ss = parse("#`PLACEHOLDER-0`{color:red}", Some(opts()));
    let Statement::QualifiedRule(qr) = &ss.statements[0] else {
        panic!("expected qualified rule");
    };
    let ComplexSelector { children, .. } = &qr.selector.selectors[0];
    let ComplexSelectorChild::CompoundSelector(compound) = &children[0] else {
        panic!("expected compound selector");
    };
    assert!(matches!(
        &compound.children[0],
        SimpleSelector::Id(IdSelector {
            name: InterpolableIdent::Placeholder(Placeholder { index: 0, .. }),
            ..
        })
    ));
}

#[test]
fn placeholder_attribute_selector_value() {
    // A placeholder in attribute value position (`[data-x=${v}]`).
    let ss = parse("a[data-x=`PLACEHOLDER-0`]{color:red}", Some(opts()));
    let Statement::QualifiedRule(qr) = &ss.statements[0] else {
        panic!("expected qualified rule");
    };
    let ComplexSelector { children, .. } = &qr.selector.selectors[0];
    let ComplexSelectorChild::CompoundSelector(compound) = &children[0] else {
        panic!("expected compound selector");
    };
    let attr = compound
        .children
        .iter()
        .find_map(|s| match s {
            SimpleSelector::Attribute(attr) => Some(attr),
            _ => None,
        })
        .expect("expected attribute selector");
    assert!(matches!(
        &attr.value,
        Some(AttributeSelectorValue::Ident(InterpolableIdent::Placeholder(Placeholder {
            index: 0,
            ..
        })))
    ));
}

#[test]
fn placeholder_led_bare_brace_is_absorbed_as_rule() {
    // A bare `{` after the placeholder (even across a newline) IS absorbed: the
    // placeholder is the selector for that block (a bare block is meaningless
    // without a selector). Matches prettier (`${mixin}\n{ ... }` -> `${mixin} {`).
    let ss = parse("`PLACEHOLDER-0`\n{ color: red }", Some(opts()));
    assert_eq!(ss.statements.len(), 1);
    assert!(matches!(ss.statements[0], Statement::QualifiedRule(_)));
}

#[test]
fn bare_placeholder_needs_no_trailing_semicolon() {
    // A `;`-less placeholder substitutes a whole `${}` interpolation; like
    // postcss it does not require a `;`, so the next statement may follow it
    // directly (`${a}\n@media {...}` / `${a} ${b}`).
    let ss = parse("`PLACEHOLDER-0`\n@media x{a{color:red}}", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::Placeholder(Placeholder { index: 0, .. })));
    assert!(matches!(ss.statements[1], Statement::AtRule(_)));

    let ss = parse("`PLACEHOLDER-0` `PLACEHOLDER-1`", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::Placeholder(Placeholder { index: 0, .. })));
    assert!(matches!(ss.statements[1], Statement::Placeholder(Placeholder { index: 1, .. })));
}

#[test]
fn placeholder_led_selector_does_not_cross_newline() {
    // A placeholder on its own source line is a standalone statement; the
    // selector on the next line is a separate rule (not one `${m} & > .x {}`).
    let ss = parse("`PLACEHOLDER-0`\n& > .x { color: red }", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::Placeholder(Placeholder { index: 0, .. })));
    assert!(matches!(ss.statements[1], Statement::QualifiedRule(_)));

    // But a same-line `${Component} { ... }` is one qualified rule.
    let ss = parse("`PLACEHOLDER-0` { color: red }", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::QualifiedRule(_)));

    // A same-line selector containing `;`/`}` inside an attribute string (or a
    // `#{...}` interpolation brace) must stay one qualified rule: the gate only
    // checks newline-crossing, the real grammar is left to QualifiedRule::parse.
    let ss = parse(r#"`PLACEHOLDER-0`[data-x="a;b"] { color: red }"#, Some(opts()));
    assert!(matches!(ss.statements[0], Statement::QualifiedRule(_)));
    assert_eq!(ss.statements.len(), 1);

    // A bare `\r` counts as a newline too (the tokenizer treats it as a line
    // break), so it splits the same way `\n` does.
    let ss = parse("`PLACEHOLDER-0`\r& > .x { color: red }", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::Placeholder(Placeholder { index: 0, .. })));
    assert!(matches!(ss.statements[1], Statement::QualifiedRule(_)));

    // BUT a placeholder immediately GLUED to non-whitespace (no space) is a
    // compound-selector piece — commas + newlines to the next placeholder
    // must NOT split it into a bare placeholder + separate rule.
    let ss = parse(
        "`PLACEHOLDER-0`:hover &,\n  `PLACEHOLDER-1`:focus-within &,\n  `PLACEHOLDER-2`:after { color: red }",
        Some(opts()),
    );
    assert_eq!(ss.statements.len(), 1);
    assert!(matches!(ss.statements[0], Statement::QualifiedRule(_)));

    // Space between placeholders is the standalone signal: two bare-placeholder
    // mixin invocations followed by a separate rule on the next line, matching
    // styled-components' `${sanitize} ${fonts}\nhtml {}` idiom.
    let ss = parse(
        "`PLACEHOLDER-0` `PLACEHOLDER-1`\nhtml { color: red }",
        Some(opts()),
    );
    assert_eq!(ss.statements.len(), 3);
    assert!(matches!(ss.statements[0], Statement::Placeholder(Placeholder { index: 0, .. })));
    assert!(matches!(ss.statements[1], Statement::Placeholder(Placeholder { index: 1, .. })));
    assert!(matches!(ss.statements[2], Statement::QualifiedRule(_)));
}

#[test]
fn placeholder_as_declaration_property_name() {
    // `${foo}: ${bar}` parses as a declaration whose property name is a
    // placeholder (not a bare placeholder statement followed by a stray `:`).
    let ss = parse("`PLACEHOLDER-0`: `PLACEHOLDER-1`;", Some(opts()));
    let Statement::Declaration(decl) = &ss.statements[0] else {
        panic!("expected declaration");
    };
    assert!(matches!(decl.name, InterpolableIdent::Placeholder(Placeholder { index: 0, .. })));
    assert!(matches!(decl.value[0], ComponentValue::Placeholder(Placeholder { index: 1, .. })));

    // A trailing `;` is optional (`${foo}: ${bar}` with no `;`).
    let ss = parse("`PLACEHOLDER-0`: `PLACEHOLDER-1`", Some(opts()));
    assert!(matches!(ss.statements[0], Statement::Declaration(_)));
}

#[test]
fn media_feature_value_placeholder_glued_to_unit() {
    // `@media (max-width: ${x}px)`: the glued unit is the placeholder's suffix,
    // so the single-value media feature slot holds one `Placeholder` (no parse
    // failure, no wrapper node).
    let ss = parse("@media (max-width:`PLACEHOLDER-0`px){a{color:red}}", Some(opts()));
    let Statement::AtRule(at_rule) = &ss.statements[0] else {
        panic!("expected at-rule");
    };
    let Some(AtRulePrelude::Media(list)) = &at_rule.prelude else {
        panic!("expected media prelude");
    };
    let MediaQuery::ConditionOnly(condition) = &list.queries[0] else {
        panic!("expected condition-only query");
    };
    let MediaConditionKind::MediaInParens(in_parens) = &condition.conditions[0] else {
        panic!("expected media-in-parens");
    };
    let MediaInParensKind::MediaFeature(feature) = &in_parens.kind else {
        panic!("expected media feature");
    };
    let MediaFeature::Plain(plain) = &**feature else {
        panic!("expected plain media feature");
    };
    assert!(matches!(
        &plain.value,
        ComponentValue::Placeholder(Placeholder { index: 0, suffix: "px", .. })
    ));
}

#[test]
fn huge_index_does_not_panic() {
    // Regression: an index that overflows u32 must not panic; it fails to match
    // the placeholder shape, so the stray backtick is a value error (not a crash).
    let allocator = Allocator::default();
    let builder = ParserBuilder::new(&allocator, "a{width:`PLACEHOLDER-9999999999`}")
        .syntax(Syntax::Scss)
        .options(opts());
    assert!(builder.build().parse::<Stylesheet>().is_err());
}

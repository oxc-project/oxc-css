//! All kinds of AST nodes are here.

use crate::{pos::Span, tokenizer::TokenWithSpan};
use oxc_allocator::{Box, Vec};
#[cfg(feature = "serialize")]
use serde::Serialize;

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct AnPlusB {
    pub a: i32,
    pub b: i32,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct AtRule<'a> {
    pub name: Ident<'a>,
    pub prelude: Option<AtRulePrelude<'a>>,
    pub block: Option<SimpleBlock<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum AtRulePrelude<'a> {
    Charset(Str<'a>),
    ColorProfile(ColorProfilePrelude<'a>),
    Container(ContainerPrelude<'a>),
    CounterStyle(InterpolableIdent<'a>),
    CustomMedia(Box<'a, CustomMedia<'a>>),
    CustomSelector(Box<'a, CustomSelectorPrelude<'a>>),
    Document(DocumentPrelude<'a>),
    FontFeatureValues(FontFamilyName<'a>),
    FontPaletteValues(InterpolableIdent<'a>),
    Import(Box<'a, ImportPrelude<'a>>),
    Keyframes(KeyframesName<'a>),
    Layer(LayerNames<'a>),
    LessImport(Box<'a, LessImportPrelude<'a>>),
    LessPlugin(Box<'a, LessPlugin<'a>>),
    Media(MediaQueryList<'a>),
    Namespace(Box<'a, NamespacePrelude<'a>>),
    Nest(SelectorList<'a>),
    Page(PageSelectorList<'a>),
    PositionTry(InterpolableIdent<'a>),
    Property(InterpolableIdent<'a>),
    SassAtRoot(SassAtRoot<'a>),
    SassContent(SassContent<'a>),
    SassEach(Box<'a, SassEach<'a>>),
    SassExpr(Box<'a, ComponentValue<'a>>),
    SassExtend(Box<'a, SassExtend<'a>>),
    SassFor(Box<'a, SassFor<'a>>),
    SassForward(Box<'a, SassForward<'a>>),
    SassFunction(Box<'a, SassFunction<'a>>),
    SassImport(SassImportPrelude<'a>),
    SassInclude(Box<'a, SassInclude<'a>>),
    SassMixin(Box<'a, SassMixin<'a>>),
    SassUse(Box<'a, SassUse<'a>>),
    Scope(Box<'a, ScopePrelude<'a>>),
    ScrollTimeline(InterpolableIdent<'a>),
    Supports(SupportsCondition<'a>),
    Unknown(Box<'a, UnknownAtRulePrelude<'a>>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct AttributeSelector<'a> {
    pub name: WqName<'a>,
    pub matcher: Option<AttributeSelectorMatcher>,
    pub value: Option<AttributeSelectorValue<'a>>,
    pub modifier: Option<AttributeSelectorModifier<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct AttributeSelectorMatcher {
    pub kind: AttributeSelectorMatcherKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum AttributeSelectorMatcherKind {
    /// `=`
    Exact,
    /// `~=`
    MatchWord,
    /// `|=`
    ExactOrPrefixThenHyphen,
    /// `^=`
    Prefix,
    /// `$=`
    Suffix,
    /// `*=`
    Substring,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct AttributeSelectorModifier<'a> {
    pub ident: InterpolableIdent<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum AttributeSelectorValue<'a> {
    Ident(InterpolableIdent<'a>),
    Str(InterpolableStr<'a>),
    Number(Number<'a>),
    Dimension(Dimension<'a>),
    Percentage(Percentage<'a>),
    LessEscapedStr(LessEscapedStr<'a>),
    TokenSeq(TokenSeq<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct BracketBlock<'a> {
    pub value: Vec<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Calc<'a> {
    pub left: Box<'a, ComponentValue<'a>>,
    pub op: CalcOperator,
    pub right: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct CalcOperator {
    pub kind: CalcOperatorKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum CalcOperatorKind {
    Plus,
    Minus,
    Multiply,
    Division,
    /// Sass modulo (`%`) inside a math function's arguments (`max(1px, 7px % 4)`).
    Modulo,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ClassSelector<'a> {
    pub name: InterpolableIdent<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum ColorProfilePrelude<'a> {
    DashedIdent(InterpolableIdent<'a>),
    DeviceCmyk(Ident<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Combinator {
    pub kind: CombinatorKind,
    pub span: Span,
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum CombinatorKind {
    /// ` `
    Descendant,
    /// `+`
    NextSibling,
    /// `>`
    Child,
    /// `~`
    LaterSibling,
    /// `||`
    Column,
    /// `/deep/` (deprecated shadow-piercing descendant)
    Deep,
    /// `^` (deprecated shadow child)
    ShadowChild,
    /// `^^` (deprecated shadow descendant)
    ShadowDescendant,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ComplexSelector<'a> {
    pub children: Vec<'a, ComplexSelectorChild<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum ComplexSelectorChild<'a> {
    CompoundSelector(CompoundSelector<'a>),
    Combinator(Combinator),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum ComponentValue<'a> {
    BracketBlock(BracketBlock<'a>),
    Calc(Calc<'a>),
    Delimiter(Delimiter),
    Dimension(Dimension<'a>),
    Function(Function<'a>),
    HexColor(HexColor<'a>),
    IdSelector(IdSelector<'a>),
    ImportantAnnotation(ImportantAnnotation<'a>),
    InterpolableIdent(InterpolableIdent<'a>),
    InterpolableStr(InterpolableStr<'a>),
    LayerName(LayerName<'a>),
    LessAnonymousMixin(LessAnonymousMixin<'a>),
    LessBinaryOperation(LessBinaryOperation<'a>),
    LessCondition(Box<'a, LessCondition<'a>>),
    LessDetachedRuleset(LessDetachedRuleset<'a>),
    LessEscapedStr(LessEscapedStr<'a>),
    LessJavaScriptSnippet(LessJavaScriptSnippet<'a>),
    LessList(LessList<'a>),
    LessMixinCall(Box<'a, LessMixinCall<'a>>),
    LessNamespaceValue(Box<'a, LessNamespaceValue<'a>>),
    LessVariableCall(LessVariableCall<'a>),
    LessNegativeValue(LessNegativeValue<'a>),
    LessParenthesizedOperation(LessParenthesizedOperation<'a>),
    LessPercentKeyword(LessPercentKeyword),
    LessPropertyVariable(LessPropertyVariable<'a>),
    LessVariable(LessVariable<'a>),
    LessVariableVariable(LessVariableVariable<'a>),
    Number(Number<'a>),
    Percentage(Percentage<'a>),
    Placeholder(Placeholder<'a>),
    PostcssSimpleVar(PostcssSimpleVar<'a>),
    Ratio(Ratio<'a>),
    SassArbitraryArgument(SassArbitraryArgument<'a>),
    SassBinaryExpression(SassBinaryExpression<'a>),
    SassKeywordArgument(SassKeywordArgument<'a>),
    SassList(SassList<'a>),
    SassMap(SassMap<'a>),
    SassQualifiedName(Box<'a, SassQualifiedName<'a>>),
    SassNestingDeclaration(SassNestingDeclaration<'a>),
    SassParenthesizedExpression(SassParenthesizedExpression<'a>),
    SassParentSelector(NestingSelector<'a>),
    SassUnaryExpression(SassUnaryExpression<'a>),
    SassVariable(SassVariable<'a>),
    TokenWithSpan(TokenWithSpan<'a>),
    UnicodeRange(UnicodeRange<'a>),
    Url(Box<'a, Url<'a>>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ComponentValues<'a> {
    pub values: Vec<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct CompoundSelector<'a> {
    pub children: Vec<'a, SimpleSelector<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct CompoundSelectorList<'a> {
    pub selectors: Vec<'a, CompoundSelector<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ContainerCondition<'a> {
    pub conditions: Vec<'a, ContainerConditionKind<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum ContainerConditionKind<'a> {
    QueryInParens(QueryInParens<'a>),
    And(ContainerConditionAnd<'a>),
    Or(ContainerConditionOr<'a>),
    Not(ContainerConditionNot<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ContainerConditionAnd<'a> {
    pub keyword: Ident<'a>,
    pub query_in_parens: QueryInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ContainerConditionNot<'a> {
    pub keyword: Ident<'a>,
    pub query_in_parens: QueryInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ContainerConditionOr<'a> {
    pub keyword: Ident<'a>,
    pub query_in_parens: QueryInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ContainerPrelude<'a> {
    pub name: Option<InterpolableIdent<'a>>,
    pub condition: ContainerCondition<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct CustomMedia<'a> {
    pub name: InterpolableIdent<'a>,
    pub value: CustomMediaValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum CustomMediaValue<'a> {
    MediaQueryList(MediaQueryList<'a>),
    True(Ident<'a>),
    False(Ident<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct CustomSelector<'a> {
    pub prefix_arg: Option<CustomSelectorArg<'a>>,
    pub name: Ident<'a>,
    pub args: Option<CustomSelectorArgs<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct CustomSelectorArg<'a> {
    pub name: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct CustomSelectorArgs<'a> {
    pub args: Vec<'a, CustomSelectorArg<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct CustomSelectorPrelude<'a> {
    pub custom_selector: CustomSelector<'a>,
    pub selector: SelectorList<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Declaration<'a> {
    pub name: InterpolableIdent<'a>,
    /// Legacy IE hack prefix glued to the property name, e.g. `'*'` in `*color: red`.
    #[cfg_attr(feature = "serialize", serde(skip_serializing_if = "Option::is_none"))]
    pub name_prefix: Option<char>,
    pub name_suffix: Option<char>,
    pub colon_span: Span,
    pub value: Vec<'a, ComponentValue<'a>>,
    pub important: Option<ImportantAnnotation<'a>>,
    pub less_property_merge: Option<LessPropertyMerge>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Delimiter {
    pub kind: DelimiterKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum DelimiterKind {
    Comma,
    Solidus,
    Semicolon,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Dimension<'a> {
    pub value: Number<'a>,
    pub unit: Ident<'a>,
    pub kind: DimensionKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum DimensionKind {
    Length,
    Angle,
    Duration,
    Frequency,
    Resolution,
    Flex,
    Unknown,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct DocumentPrelude<'a> {
    pub matchers: Vec<'a, DocumentPreludeMatcher<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum DocumentPreludeMatcher<'a> {
    Url(Url<'a>),
    Function(Function<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum FontFamilyName<'a> {
    Str(InterpolableStr<'a>),
    Unquoted(UnquotedFontFamilyName<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Function<'a> {
    pub name: FunctionName<'a>,
    pub args: Vec<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum FunctionName<'a> {
    Ident(InterpolableIdent<'a>),
    SassQualifiedName(Box<'a, SassQualifiedName<'a>>),
    LessListFunction(LessListFunction),
    LessFormatFunction(LessFormatFunction),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct HexColor<'a> {
    pub value: &'a str,
    pub raw: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Ident<'a> {
    pub name: &'a str,
    pub raw: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ImportPrelude<'a> {
    pub href: ImportPreludeHref<'a>,
    pub layer: Option<ImportPreludeLayer<'a>>,
    pub supports: Option<ImportPreludeSupports<'a>>,
    pub media: Option<MediaQueryList<'a>>,
    /// Import modifiers that don't fit the `layer()`/`supports()`/media-query
    /// grammar, kept as raw component values — reference compilers accept an
    /// arbitrary mix of idents, functions, and parens here, and a comma may
    /// even chain further imports (`@import "a" b c(d), "e" supports(f: g);`).
    /// When present, `layer`/`supports`/`media` are `None` and the whole
    /// post-URL tail lives here.
    pub modifiers: Option<ComponentValues<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum ImportPreludeHref<'a> {
    Str(InterpolableStr<'a>),
    Url(Url<'a>),
    /// Sass only: `url(...)` whose content is not a parsable URL but is
    /// valid SassScript, e.g. `@import url($dir+"/path");`.
    Function(Function<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum ImportPreludeLayer<'a> {
    Empty(Ident<'a>),
    WithName(Function<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ImportPreludeSupports<'a> {
    pub kind: ImportPreludeSupportsKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum ImportPreludeSupportsKind<'a> {
    SupportsCondition(SupportsCondition<'a>),
    Declaration(Declaration<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum InterpolableIdent<'a> {
    Literal(Ident<'a>),
    SassInterpolated(SassInterpolatedIdent<'a>),
    LessInterpolated(LessInterpolatedIdent<'a>),
    Placeholder(Placeholder<'a>),
}

/// An atomic backtick-delimited template placeholder node (see
/// [`ParserOptions::template_placeholder`](crate::config::ParserOptions)),
/// carrying the parsed decimal index. Appears in value, selector, and
/// statement positions where a downstream formatter substitutes interpolations.
#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Placeholder<'a> {
    pub index: u32,
    /// An ident-continuation run glued directly after the placeholder
    /// (`` `PLACEHOLDER-0`px `` -> index 0, suffix `"px"`), empty when none.
    /// Mirrors `#{$x}px` being a single identifier rather than two tokens.
    pub suffix: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct InterpolableIdentStaticPart<'a> {
    pub value: &'a str,
    pub raw: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum InterpolableStr<'a> {
    Literal(Str<'a>),
    SassInterpolated(SassInterpolatedStr<'a>),
    LessInterpolated(LessInterpolatedStr<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct InterpolableStrStaticPart<'a> {
    pub value: &'a str,
    pub raw: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct InterpolableUrlStaticPart<'a> {
    pub value: &'a str,
    pub raw: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct IdSelector<'a> {
    pub name: InterpolableIdent<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ImportantAnnotation<'a> {
    pub ident: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct KeyframeBlock<'a> {
    pub selectors: Vec<'a, KeyframeSelector<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub block: SimpleBlock<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum KeyframeSelector<'a> {
    Ident(InterpolableIdent<'a>),
    Percentage(Percentage<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum KeyframesName<'a> {
    Ident(InterpolableIdent<'a>),
    Str(InterpolableStr<'a>),
    LessVariable(LessVariable<'a>),
    LessEscapedStr(LessEscapedStr<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LanguageRange<'a> {
    Str(InterpolableStr<'a>),
    Ident(InterpolableIdent<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LanguageRangeList<'a> {
    pub ranges: Vec<'a, LanguageRange<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LayerName<'a> {
    pub idents: Vec<'a, InterpolableIdent<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LayerNames<'a> {
    pub names: Vec<'a, LayerName<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessBinaryCondition<'a> {
    pub left: Box<'a, LessCondition<'a>>,
    pub op: LessBinaryConditionOperator,
    pub right: Box<'a, LessCondition<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessBinaryConditionOperator {
    pub kind: LessBinaryConditionOperatorKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum LessBinaryConditionOperatorKind {
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Equal,
    EqualOrGreaterThan,
    EqualOrLessThan,
    And,
    Or,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessAnonymousMixin<'a> {
    pub params: LessMixinParameters<'a>,
    pub block: SimpleBlock<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessBinaryOperation<'a> {
    pub left: Box<'a, ComponentValue<'a>>,
    pub op: LessOperationOperator,
    pub right: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessCondition<'a> {
    Binary(LessBinaryCondition<'a>),
    Negated(LessNegatedCondition<'a>),
    Parenthesized(LessParenthesizedCondition<'a>),
    Value(ComponentValue<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessConditionalQualifiedRule<'a> {
    pub selector: SelectorList<'a>,
    pub guard: LessConditions<'a>,
    pub block: SimpleBlock<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessConditions<'a> {
    pub conditions: Vec<'a, LessCondition<'a>>,
    pub when_span: Span,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessDetachedRuleset<'a> {
    pub block: SimpleBlock<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessEscapedStr<'a> {
    pub str: Str<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessExtend<'a> {
    pub selector: ComplexSelector<'a>,
    pub all: Option<Ident<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessExtendList<'a> {
    pub elements: Vec<'a, LessExtend<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessExtendRule<'a> {
    pub nesting_selector: NestingSelector<'a>,
    pub name_of_extend: Ident<'a>,
    pub extend: LessExtendList<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessFormatFunction {
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessImportOptions<'a> {
    pub names: Vec<'a, Ident<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessImportPrelude<'a> {
    pub href: ImportPreludeHref<'a>,
    pub options: LessImportOptions<'a>,
    pub media: Option<MediaQueryList<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessInterpolatedIdent<'a> {
    pub elements: Vec<'a, LessInterpolatedIdentElement<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessInterpolatedIdentElement<'a> {
    Variable(LessVariableInterpolation<'a>),
    Property(LessPropertyInterpolation<'a>),
    Static(InterpolableIdentStaticPart<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessInterpolatedStr<'a> {
    pub elements: Vec<'a, LessInterpolatedStrElement<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessInterpolatedStrElement<'a> {
    Variable(LessVariableInterpolation<'a>),
    Property(LessPropertyInterpolation<'a>),
    Static(InterpolableStrStaticPart<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessJavaScriptSnippet<'a> {
    pub code: &'a str,
    pub raw: &'a str,
    pub escaped: bool,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessList<'a> {
    pub elements: Vec<'a, ComponentValue<'a>>,
    pub comma_spans: Option<Vec<'a, Span>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessListFunction {
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessLookup<'a> {
    pub name: Option<LessLookupName<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessLookupName<'a> {
    LessVariable(LessVariable<'a>),
    LessVariableVariable(LessVariableVariable<'a>),
    LessPropertyVariable(LessPropertyVariable<'a>),
    LessPropertyInterpolation(LessPropertyInterpolation<'a>),
    Ident(Ident<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessLookups<'a> {
    pub lookups: Vec<'a, LessLookup<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessMixinArgument<'a> {
    Named(LessMixinNamedArgument<'a>),
    Value(ComponentValue<'a>),
    Variadic(LessMixinVariadicArgument<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinArguments<'a> {
    pub args: Vec<'a, LessMixinArgument<'a>>,
    pub is_comma_separated: bool,
    pub separator_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinCall<'a> {
    pub callee: LessMixinCallee<'a>,
    pub args: Option<LessMixinArguments<'a>>,
    pub important: Option<ImportantAnnotation<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinCallee<'a> {
    pub children: Vec<'a, LessMixinCalleeChild<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinCalleeChild<'a> {
    pub name: LessMixinName<'a>,
    pub combinator: Option<Combinator>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinDefinition<'a> {
    pub name: LessMixinName<'a>,
    pub params: LessMixinParameters<'a>,
    pub guard: Option<LessConditions<'a>>,
    pub block: SimpleBlock<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessMixinName<'a> {
    ClassSelector(ClassSelector<'a>),
    IdSelector(IdSelector<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinNamedArgument<'a> {
    pub name: LessMixinParameterName<'a>,
    pub colon_span: Span,
    pub value: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinNamedParameter<'a> {
    pub name: LessMixinParameterName<'a>,
    pub value: Option<LessMixinNamedParameterDefaultValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinNamedParameterDefaultValue<'a> {
    pub colon_span: Span,
    pub value: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessMixinParameter<'a> {
    Named(LessMixinNamedParameter<'a>),
    Unnamed(LessMixinUnnamedParameter<'a>),
    Variadic(LessMixinVariadicParameter<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinParameters<'a> {
    pub params: Vec<'a, LessMixinParameter<'a>>,
    pub is_comma_separated: bool,
    pub separator_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessMixinParameterName<'a> {
    Variable(LessVariable<'a>),
    PropertyVariable(LessPropertyVariable<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinUnnamedParameter<'a> {
    pub value: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinVariadicArgument<'a> {
    pub name: LessMixinParameterName<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessMixinVariadicParameter<'a> {
    pub name: Option<LessMixinParameterName<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessNamespaceValue<'a> {
    pub callee: LessNamespaceValueCallee<'a>,
    pub lookups: LessLookups<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessNamespaceValueCallee<'a> {
    LessMixinCall(LessMixinCall<'a>),
    LessVariable(LessVariable<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessNegatedCondition<'a> {
    pub condition: Box<'a, LessCondition<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessNegativeValue<'a> {
    pub value: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessOperationOperator {
    pub kind: LessOperationOperatorKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum LessOperationOperatorKind {
    Multiply,
    Division,
    Plus,
    Minus,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessParenthesizedCondition<'a> {
    pub condition: Box<'a, LessCondition<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessParenthesizedOperation<'a> {
    pub operation: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessPercentKeyword {
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessPlugin<'a> {
    pub path: LessPluginPath<'a>,
    pub args: Option<TokenSeq<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum LessPluginPath<'a> {
    Str(Str<'a>),
    Url(Url<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessPropertyInterpolation<'a> {
    pub name: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessPropertyMerge {
    pub kind: LessPropertyMergeKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum LessPropertyMergeKind {
    Comma,
    Space,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessPropertyVariable<'a> {
    pub name: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessVariable<'a> {
    pub name: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessVariableCall<'a> {
    pub variable: LessVariable<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessVariableDeclaration<'a> {
    pub name: LessVariable<'a>,
    pub colon_span: Span,
    pub value: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessVariableInterpolation<'a> {
    pub name: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct LessVariableVariable<'a> {
    pub variable: LessVariable<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaAnd<'a> {
    pub keyword: Ident<'a>,
    pub media_in_parens: MediaInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaCondition<'a> {
    pub conditions: Vec<'a, MediaConditionKind<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaConditionAfterMediaType<'a> {
    pub and: Ident<'a>,
    pub condition: MediaCondition<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum MediaConditionKind<'a> {
    MediaInParens(MediaInParens<'a>),
    And(MediaAnd<'a>),
    Or(MediaOr<'a>),
    Not(MediaNot<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum MediaFeature<'a> {
    Plain(MediaFeaturePlain<'a>),
    Boolean(MediaFeatureBoolean<'a>),
    Range(MediaFeatureRange<'a>),
    RangeInterval(MediaFeatureRangeInterval<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaFeatureComparison {
    pub kind: MediaFeatureComparisonKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum MediaFeatureComparisonKind {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum MediaFeatureName<'a> {
    Ident(InterpolableIdent<'a>),
    PostcssSimpleVar(PostcssSimpleVar<'a>),
    SassVariable(SassVariable<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaFeatureBoolean<'a> {
    pub name: MediaFeatureName<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaFeaturePlain<'a> {
    pub name: MediaFeatureName<'a>,
    pub colon_span: Span,
    pub value: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaFeatureRange<'a> {
    pub left: ComponentValue<'a>,
    pub comparison: MediaFeatureComparison,
    pub right: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaFeatureRangeInterval<'a> {
    pub left: ComponentValue<'a>,
    pub left_comparison: MediaFeatureComparison,
    pub name: MediaFeatureName<'a>,
    pub right_comparison: MediaFeatureComparison,
    pub right: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaInParens<'a> {
    pub kind: MediaInParensKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum MediaInParensKind<'a> {
    MediaCondition(MediaCondition<'a>),
    MediaFeature(Box<'a, MediaFeature<'a>>),
    GeneralEnclosed(TokenSeq<'a>),
    SassInterpolation(SassInterpolatedIdent<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaNot<'a> {
    pub keyword: Ident<'a>,
    pub media_in_parens: MediaInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaOr<'a> {
    pub keyword: Ident<'a>,
    pub media_in_parens: MediaInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum MediaQuery<'a> {
    ConditionOnly(MediaCondition<'a>),
    WithType(MediaQueryWithType<'a>),
    Function(Function<'a>),
    LessVariable(LessVariable<'a>),
    LessNamespaceValue(Box<'a, LessNamespaceValue<'a>>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaQueryList<'a> {
    pub queries: Vec<'a, MediaQuery<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct MediaQueryWithType<'a> {
    pub modifier: Option<Ident<'a>>,
    pub media_type: InterpolableIdent<'a>,
    pub condition: Option<MediaConditionAfterMediaType<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct NamespacePrelude<'a> {
    pub prefix: Option<InterpolableIdent<'a>>,
    pub uri: NamespacePreludeUri<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum NamespacePreludeUri<'a> {
    Str(InterpolableStr<'a>),
    Url(Url<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct NestingSelector<'a> {
    pub suffix: Option<InterpolableIdent<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct NsPrefix<'a> {
    pub kind: Option<NsPrefixKind<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum NsPrefixKind<'a> {
    Ident(InterpolableIdent<'a>),
    Universal(NsPrefixUniversal),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct NsPrefixUniversal {
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Nth<'a> {
    pub index: NthIndex<'a>,
    pub matcher: Option<NthMatcher<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum NthIndex<'a> {
    Odd(Ident<'a>),
    Even(Ident<'a>),
    Integer(Number<'a>),
    AnPlusB(AnPlusB),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct NthMatcher<'a> {
    pub selector: Option<SelectorList<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Number<'a> {
    pub value: f32,
    pub raw: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PageSelector<'a> {
    pub name: Option<InterpolableIdent<'a>>,
    pub pseudo: Vec<'a, PseudoPage<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PageSelectorList<'a> {
    pub selectors: Vec<'a, PageSelector<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Percentage<'a> {
    pub value: Number<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PostcssSimpleVar<'a> {
    pub name: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PostcssSimpleVarDeclaration<'a> {
    pub name: PostcssSimpleVar<'a>,
    pub colon_span: Span,
    pub value: Vec<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PseudoClassSelector<'a> {
    pub name: InterpolableIdent<'a>,
    pub arg: Option<PseudoClassSelectorArg<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PseudoClassSelectorArg<'a> {
    pub kind: PseudoClassSelectorArgKind<'a>,
    pub l_paren: Span,
    pub r_paren: Span,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum PseudoClassSelectorArgKind<'a> {
    CompoundSelector(CompoundSelector<'a>),
    CompoundSelectorList(CompoundSelectorList<'a>),
    Ident(InterpolableIdent<'a>),
    LanguageRangeList(LanguageRangeList<'a>),
    Nth(Nth<'a>),
    Number(Number<'a>),
    RelativeSelectorList(RelativeSelectorList<'a>),
    SelectorList(SelectorList<'a>),
    LessExtendList(LessExtendList<'a>),
    TokenSeq(TokenSeq<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PseudoElementSelector<'a> {
    pub name: InterpolableIdent<'a>,
    pub arg: Option<PseudoElementSelectorArg<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PseudoElementSelectorArg<'a> {
    pub kind: PseudoElementSelectorArgKind<'a>,
    pub l_paren: Span,
    pub r_paren: Span,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum PseudoElementSelectorArgKind<'a> {
    CompoundSelector(CompoundSelector<'a>),
    CompoundSelectorList(CompoundSelectorList<'a>),
    Ident(InterpolableIdent<'a>),
    TokenSeq(TokenSeq<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct PseudoPage<'a> {
    pub name: InterpolableIdent<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct QualifiedRule<'a> {
    pub selector: SelectorList<'a>,
    pub block: SimpleBlock<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct QueryInParens<'a> {
    pub kind: QueryInParensKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum QueryInParensKind<'a> {
    ContainerCondition(ContainerCondition<'a>),
    SizeFeature(Box<'a, MediaFeature<'a>>),
    StyleQuery(StyleQuery<'a>),
    ScrollState(Box<'a, MediaFeature<'a>>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Ratio<'a> {
    pub numerator: Number<'a>,
    pub solidus_span: Span,
    pub denominator: Number<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct RelativeSelector<'a> {
    pub combinator: Option<Combinator>,
    pub complex_selector: ComplexSelector<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct RelativeSelectorList<'a> {
    pub selectors: Vec<'a, RelativeSelector<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassArbitraryArgument<'a> {
    pub value: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassArbitraryParameter<'a> {
    pub name: SassVariable<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassAtRoot<'a> {
    pub kind: SassAtRootKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SassAtRootKind<'a> {
    Selector(SelectorList<'a>),
    Query(SassAtRootQuery<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassAtRootQuery<'a> {
    pub modifier: SassAtRootQueryModifier,
    pub colon_span: Span,
    /// space-separated rule names
    pub rules: Vec<'a, SassAtRootQueryRule<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassAtRootQueryModifier {
    pub kind: SassAtRootQueryModifierKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum SassAtRootQueryModifierKind {
    With,
    Without,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SassAtRootQueryRule<'a> {
    Ident(InterpolableIdent<'a>),
    Str(InterpolableStr<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassBinaryExpression<'a> {
    pub left: Box<'a, ComponentValue<'a>>,
    pub op: SassBinaryOperator,
    pub right: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassBinaryOperator {
    pub kind: SassBinaryOperatorKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum SassBinaryOperatorKind {
    Multiply,
    Division,
    Modulo,
    Plus,
    Minus,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    EqualsEquals,
    ExclamationEquals,
    And,
    Or,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassConditionalClause<'a> {
    pub condition: ComponentValue<'a>,
    pub block: SimpleBlock<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassContent<'a> {
    pub args: Vec<'a, ComponentValue<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassEach<'a> {
    pub bindings: Vec<'a, SassVariable<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub in_span: Span,
    pub expr: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassExtend<'a> {
    pub selectors: CompoundSelectorList<'a>,
    pub optional: Option<SassFlag<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassFlag<'a> {
    pub keyword: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassFor<'a> {
    pub binding: SassVariable<'a>,
    pub from_span: Span,
    pub start: ComponentValue<'a>,
    pub end: ComponentValue<'a>,
    pub boundary: SassForBoundary,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassForBoundary {
    pub kind: SassForBoundaryKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum SassForBoundaryKind {
    Inclusive,
    Exclusive,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassForward<'a> {
    pub path: InterpolableStr<'a>,
    pub prefix: Option<SassForwardPrefix<'a>>,
    pub visibility: Option<SassForwardVisibility<'a>>,
    pub config: Option<SassModuleConfig<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SassForwardMember<'a> {
    Ident(Ident<'a>),
    Variable(SassVariable<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassForwardPrefix<'a> {
    pub as_span: Span,
    pub name: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassForwardVisibility<'a> {
    pub modifier: SassForwardVisibilityModifier,
    pub members: Vec<'a, SassForwardMember<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassForwardVisibilityModifier {
    pub kind: SassForwardVisibilityModifierKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum SassForwardVisibilityModifierKind {
    Hide,
    Show,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassFunction<'a> {
    pub name: Ident<'a>,
    pub parameters: SassParameters<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassIfAtRule<'a> {
    pub if_clause: SassConditionalClause<'a>,
    pub else_if_clauses: Vec<'a, SassConditionalClause<'a>>,
    pub else_clause: Option<SimpleBlock<'a>>,
    pub else_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassImportPrelude<'a> {
    pub paths: Vec<'a, Str<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassInclude<'a> {
    pub name: FunctionName<'a>,
    pub arguments: Option<SassIncludeArgs<'a>>,
    pub content_block_params: Option<SassIncludeContentBlockParams<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassIncludeArgs<'a> {
    pub args: Vec<'a, ComponentValue<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassIncludeContentBlockParams<'a> {
    pub using_span: Span,
    pub params: SassParameters<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassInterpolatedIdent<'a> {
    pub elements: Vec<'a, SassInterpolatedIdentElement<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SassInterpolatedIdentElement<'a> {
    Expression(ComponentValue<'a>),
    Static(InterpolableIdentStaticPart<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassInterpolatedStr<'a> {
    pub elements: Vec<'a, SassInterpolatedStrElement<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SassInterpolatedStrElement<'a> {
    Expression(ComponentValue<'a>),
    Static(InterpolableStrStaticPart<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassInterpolatedUrl<'a> {
    pub elements: Vec<'a, SassInterpolatedUrlElement<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SassInterpolatedUrlElement<'a> {
    Expression(ComponentValue<'a>),
    Static(InterpolableUrlStaticPart<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassKeywordArgument<'a> {
    pub name: SassVariable<'a>,
    pub colon_span: Span,
    pub value: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassList<'a> {
    pub elements: Vec<'a, ComponentValue<'a>>,
    pub comma_spans: Option<Vec<'a, Span>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassMap<'a> {
    pub items: Vec<'a, SassMapItem<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassMapItem<'a> {
    pub key: ComponentValue<'a>,
    pub colon_span: Span,
    pub value: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassMixin<'a> {
    pub name: Ident<'a>,
    pub parameters: Option<SassParameters<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassModuleConfig<'a> {
    pub with_span: Span,
    pub lparen_span: Span,
    pub items: Vec<'a, SassModuleConfigItem<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassModuleConfigItem<'a> {
    pub variable: SassVariable<'a>,
    pub colon_span: Span,
    pub value: ComponentValue<'a>,
    pub flags: Vec<'a, SassFlag<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SassModuleMemberName<'a> {
    Ident(Ident<'a>),
    Variable(SassVariable<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassNestingDeclaration<'a> {
    pub block: SimpleBlock<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassParameter<'a> {
    pub name: SassVariable<'a>,
    pub default_value: Option<SassParameterDefaultValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassParameterDefaultValue<'a> {
    pub colon_span: Span,
    pub value: ComponentValue<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassParameters<'a> {
    pub params: Vec<'a, SassParameter<'a>>,
    pub arbitrary_param: Option<SassArbitraryParameter<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassParenthesizedExpression<'a> {
    pub expr: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassPlaceholderSelector<'a> {
    pub name: InterpolableIdent<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassQualifiedName<'a> {
    pub module: Ident<'a>,
    pub member: SassModuleMemberName<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassUnaryExpression<'a> {
    pub op: SassUnaryOperator,
    pub expr: Box<'a, ComponentValue<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassUnaryOperator {
    pub kind: SassUnaryOperatorKind,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub enum SassUnaryOperatorKind {
    Plus,
    Minus,
    Not,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassUnnamedNamespace {
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassUse<'a> {
    pub path: InterpolableStr<'a>,
    pub namespace: Option<SassUseNamespace<'a>>,
    pub config: Option<SassModuleConfig<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassUseNamespace<'a> {
    pub as_span: Span,
    pub kind: SassUseNamespaceKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SassUseNamespaceKind<'a> {
    Named(Ident<'a>),
    Unnamed(SassUnnamedNamespace),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassVariable<'a> {
    pub name: Ident<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SassVariableDeclaration<'a> {
    pub namespace: Option<Ident<'a>>,
    pub name: SassVariable<'a>,
    pub colon_span: Span,
    pub value: ComponentValue<'a>,
    pub flags: Vec<'a, SassFlag<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ScopeEnd<'a> {
    pub to_span: Span,
    pub lparen_span: Span,
    pub selector: SelectorList<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum ScopePrelude<'a> {
    StartOnly(ScopeStart<'a>),
    EndOnly(ScopeEnd<'a>),
    Both(ScopeStartWithEnd<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ScopeStart<'a> {
    pub selector: SelectorList<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct ScopeStartWithEnd<'a> {
    pub start: ScopeStart<'a>,
    pub end: ScopeEnd<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SelectorList<'a> {
    pub selectors: Vec<'a, ComplexSelector<'a>>,
    pub comma_spans: Vec<'a, Span>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SimpleBlock<'a> {
    pub statements: Vec<'a, Statement<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SimpleSelector<'a> {
    Class(ClassSelector<'a>),
    Id(IdSelector<'a>),
    Type(TypeSelector<'a>),
    Attribute(AttributeSelector<'a>),
    PseudoClass(PseudoClassSelector<'a>),
    PseudoElement(PseudoElementSelector<'a>),
    Nesting(NestingSelector<'a>),
    SassPlaceholder(SassPlaceholderSelector<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum Statement<'a> {
    AtRule(AtRule<'a>),
    Declaration(Declaration<'a>),
    KeyframeBlock(KeyframeBlock<'a>),
    LessConditionalQualifiedRule(LessConditionalQualifiedRule<'a>),
    LessExtendRule(LessExtendRule<'a>),
    LessFunctionCall(Function<'a>),
    LessMixinCall(LessMixinCall<'a>),
    LessMixinDefinition(Box<'a, LessMixinDefinition<'a>>),
    LessVariableCall(LessVariableCall<'a>),
    LessVariableDeclaration(Box<'a, LessVariableDeclaration<'a>>),
    Placeholder(Placeholder<'a>),
    PostcssSimpleVarDeclaration(Box<'a, PostcssSimpleVarDeclaration<'a>>),
    QualifiedRule(QualifiedRule<'a>),
    SassIfAtRule(Box<'a, SassIfAtRule<'a>>),
    SassVariableDeclaration(Box<'a, SassVariableDeclaration<'a>>),
    UnknownSassAtRule(Box<'a, UnknownSassAtRule<'a>>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Str<'a> {
    pub value: &'a str,
    pub raw: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct StyleCondition<'a> {
    pub conditions: Vec<'a, StyleConditionKind<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum StyleConditionKind<'a> {
    StyleInParens(StyleInParens<'a>),
    And(StyleConditionAnd<'a>),
    Or(StyleConditionOr<'a>),
    Not(StyleConditionNot<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct StyleConditionAnd<'a> {
    pub keyword: Ident<'a>,
    pub style_in_parens: StyleInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct StyleConditionNot<'a> {
    pub keyword: Ident<'a>,
    pub style_in_parens: StyleInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct StyleConditionOr<'a> {
    pub keyword: Ident<'a>,
    pub style_in_parens: StyleInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct StyleInParens<'a> {
    pub kind: StyleInParensKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum StyleInParensKind<'a> {
    Condition(StyleCondition<'a>),
    Feature(Declaration<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum StyleQuery<'a> {
    Condition(StyleCondition<'a>),
    Feature(Declaration<'a>),
    /// A bare `<custom-property-name>` existence test: `style(--theme)`
    /// (css-conditional-5 `<style-feature>`).
    FeatureName(InterpolableIdent<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Stylesheet<'a> {
    pub statements: Vec<'a, Statement<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SupportsAnd<'a> {
    pub keyword: Ident<'a>,
    pub condition: SupportsInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SupportsCondition<'a> {
    pub conditions: Vec<'a, SupportsConditionKind<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SupportsConditionKind<'a> {
    Not(SupportsNot<'a>),
    And(SupportsAnd<'a>),
    Or(SupportsOr<'a>),
    SupportsInParens(SupportsInParens<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SupportsDecl<'a> {
    pub decl: Declaration<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SupportsInParens<'a> {
    pub kind: SupportsInParensKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum SupportsInParensKind<'a> {
    SupportsCondition(SupportsCondition<'a>),
    Feature(Box<'a, SupportsDecl<'a>>),
    Selector(SelectorList<'a>),
    Function(Function<'a>),
    GeneralEnclosed(TokenSeq<'a>),
    /// Sass only: a lone interpolation as a condition operand,
    /// e.g. `@supports #{"(a: b)"} { ... }`.
    Interpolation(InterpolableIdent<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SupportsNot<'a> {
    pub keyword: Ident<'a>,
    pub condition: SupportsInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct SupportsOr<'a> {
    pub keyword: Ident<'a>,
    pub condition: SupportsInParens<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct TagNameSelector<'a> {
    pub name: WqName<'a>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct TokenSeq<'a> {
    pub tokens: Vec<'a, TokenWithSpan<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum TypeSelector<'a> {
    TagName(TagNameSelector<'a>),
    Universal(UniversalSelector<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct UnicodeRange<'a> {
    pub prefix: char,
    pub start: u32,
    pub start_raw: &'a str,
    pub end: u32,
    pub end_raw: Option<&'a str>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct UniversalSelector<'a> {
    pub prefix: Option<NsPrefix<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum UnknownAtRulePrelude<'a> {
    ComponentValue(ComponentValue<'a>),
    TokenSeq(TokenSeq<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct UnknownSassAtRule<'a> {
    pub name: InterpolableIdent<'a>,
    pub prelude: Option<UnknownAtRulePrelude<'a>>,
    pub block: Option<SimpleBlock<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct UnquotedFontFamilyName<'a> {
    pub idents: Vec<'a, InterpolableIdent<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct Url<'a> {
    pub name: Ident<'a>,
    pub value: Option<UrlValue<'a>>,
    pub modifiers: Vec<'a, UrlModifier<'a>>,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum UrlModifier<'a> {
    Ident(InterpolableIdent<'a>),
    Function(Function<'a>),
}

/// `)` is excluded
#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct UrlRaw<'a> {
    pub value: &'a str,
    pub raw: &'a str,
    pub span: Span,
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(untagged))]
pub enum UrlValue<'a> {
    Raw(UrlRaw<'a>),
    SassInterpolated(SassInterpolatedUrl<'a>),
    Str(InterpolableStr<'a>),
    LessEscapedStr(LessEscapedStr<'a>),
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "serialize", serde(tag = "type", rename_all = "camelCase"))]
pub struct WqName<'a> {
    pub name: InterpolableIdent<'a>,
    pub prefix: Option<NsPrefix<'a>>,
    pub span: Span,
}

include!("ast_generated.rs");

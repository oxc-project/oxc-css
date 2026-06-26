use oxc_css_parser::{Allocator, Parser, ast::Stylesheet};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = parseStylesheet)]
pub fn parse_stylesheet(source: String, syntax: JsValue) -> Result<JsValue, JsValue> {
    let syntax = serde_wasm_bindgen::from_value(syntax)?;
    let allocator = Allocator::default();
    let mut parser = Parser::new(&allocator, &source, syntax);
    match parser.parse::<Stylesheet>() {
        Ok(ast) => {
            let serializer = serde_wasm_bindgen::Serializer::new().serialize_missing_as_null(true);
            ast.serialize(&serializer).map_err(JsValue::from)
        }
        Err(error) => Err(serde_wasm_bindgen::to_value(&(&error, error.kind.to_string()))?),
    }
}

use oxc_css_parser::{Allocator, ParserBuilder, ast::Stylesheet};

fn main() {
    let allocator = Allocator::default();
    let mut parser = ParserBuilder::new(
        &allocator,
        "
a {
    /* comment */
    color: green;
}
    ",
    )
    .comments()
    .build();
    let _ = parser.parse::<Stylesheet>().unwrap();
    println!("{:#?}", parser.comments());
}

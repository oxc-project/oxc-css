# oxc-css-parser

[![Crates.io](https://img.shields.io/crates/v/oxc-css-parser?style=flat-square)](https://crates.io/crates/oxc-css-parser)
[![docs.rs](https://img.shields.io/docsrs/oxc-css-parser?style=flat-square)](https://docs.rs/oxc-css-parser)

`oxc-css-parser` parses CSS, SCSS, Sass, and Less. It produces an AST and does not compile preprocessor syntax to CSS.

## Example

```rust
use oxc_css_parser::{Allocator, Parser, Syntax, ast::Stylesheet};

let allocator = Allocator::default();
let mut parser = Parser::new(&allocator, "a { color: green }", Syntax::Css);
let ast = parser.parse::<Stylesheet>().unwrap();
println!("{:#?}", ast);
```

More examples are available in [`examples`](https://github.com/oxc-project/oxc-css-parser/tree/main/examples).

For detailed API documentation, see [docs.rs](https://docs.rs/oxc-css-parser).

## Benchmark

The benchmark suite compares parser performance against other CSS parsers.

Install `cargo-criterion`, then add CSS files to a local `bench_data` directory:

```sh
cargo install cargo-criterion
cargo criterion
```

## Credits

Tests come from:

- [Web Platform Tests](https://github.com/web-platform-tests/wpt)
- [SWC CSS parser](https://github.com/swc-project/swc/tree/main/crates/swc_css_parser/tests)
- [ESBuild](https://github.com/evanw/esbuild/blob/master/internal/css_parser/css_parser_test.go)
- [Sass Spec Suite](https://github.com/sass/sass-spec)
- [Less Test Suite](https://github.com/less/less.js/tree/master/packages/test-data)

## License

MIT License

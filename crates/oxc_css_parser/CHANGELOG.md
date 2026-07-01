# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.3](https://github.com/oxc-project/oxc-css-parser/compare/oxc-css-parser-v0.0.2...oxc-css-parser-v0.0.3) - 2026-07-01

### Fixed

- glued placeholder handling ([#25](https://github.com/oxc-project/oxc-css-parser/pull/25))
- accept unquoted numeric attribute selector values like [size=1] ([#21](https://github.com/oxc-project/oxc-css-parser/pull/21))

## [0.0.2](https://github.com/oxc-project/oxc-css-parser/compare/oxc-css-parser-v0.0.1...oxc-css-parser-v0.0.2) - 2026-06-30

### Added

- support <general-enclosed> in @media and @supports ([#15](https://github.com/oxc-project/oxc-css-parser/pull/15))

### Fixed

- adapt to oxc_allocator 0.138.0 GetAllocator API

### Other

- speed up sass expression parsing ([#10](https://github.com/oxc-project/oxc-css-parser/pull/10))
- box oversized Sass/Less variants of Statement ([#12](https://github.com/oxc-project/oxc-css-parser/pull/12))
- box oversized ComponentValue variants ([#11](https://github.com/oxc-project/oxc-css-parser/pull/11))
- make parser benchmark self-contained ([#9](https://github.com/oxc-project/oxc-css-parser/pull/9))
- remove parser macro crate ([#8](https://github.com/oxc-project/oxc-css-parser/pull/8))
- allocate AST in oxc_allocator ([#7](https://github.com/oxc-project/oxc-css-parser/pull/7))
- add workspace lint configuration
- move examples and benches to workspace root
- move benchmark into parser crate
- fix clippy warnings
- remove copyright notices

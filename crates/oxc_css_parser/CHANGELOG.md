# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.4](https://github.com/oxc-project/oxc-css-parser/compare/oxc-css-parser-v0.0.3...oxc-css-parser-v0.0.4) - 2026-07-02

### Added

- fix remaining conformance failures (1352 -> 49) ([#60](https://github.com/oxc-project/oxc-css-parser/pull/60))
- accept consecutive/leading/trailing combinators in Sass selectors ([#50](https://github.com/oxc-project/oxc-css-parser/pull/50))
- accept `;` as a statement terminator in the indented syntax ([#49](https://github.com/oxc-project/oxc-css-parser/pull/49))

### Fixed

- accept unusual attribute selector values like `[attr=;]` ([#36](https://github.com/oxc-project/oxc-css-parser/pull/36)) ([#63](https://github.com/oxc-project/oxc-css-parser/pull/63))
- media and-ident leniency, Less numeric properties, dart-sass todo tests ([#62](https://github.com/oxc-project/oxc-css-parser/pull/62))
- resolve remaining conformance failures (49 → 20) ([#61](https://github.com/oxc-project/oxc-css-parser/pull/61))
- ignore indentation inside `(...)` in the indented syntax ([#48](https://github.com/oxc-project/oxc-css-parser/pull/48))
- allow a nested `@import` to be terminated by `}` (no trailing `;`) ([#47](https://github.com/oxc-project/oxc-css-parser/pull/47))
- parse an empty CSS rule with no selector (`{}`) ([#46](https://github.com/oxc-project/oxc-css-parser/pull/46))
- parse Sass `%` modulo inside math function arguments ([#45](https://github.com/oxc-project/oxc-css-parser/pull/45))
- parse Less `@3` and `@{3}` digit-led variable names ([#44](https://github.com/oxc-project/oxc-css-parser/pull/44))
- parse leading `*` IE property hack (`*color: red`) ([#41](https://github.com/oxc-project/oxc-css-parser/pull/41))
- parse `@import` with no media query at end of input ([#40](https://github.com/oxc-project/oxc-css-parser/pull/40))
- don't panic on `@-`/`$-` (at-keyword/variable with a bare `-`) ([#38](https://github.com/oxc-project/oxc-css-parser/pull/38))

### Other

- assert parses instead of snapshotting ASTs ([#43](https://github.com/oxc-project/oxc-css-parser/pull/43))

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

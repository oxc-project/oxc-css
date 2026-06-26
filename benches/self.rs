use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use oxc_css_parser::{Allocator, Parser, Syntax, ast::Stylesheet};
use std::{fs, hint::black_box, path::Path, time::Duration};

fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("self");
    group.measurement_time(Duration::from_secs(12));

    let bench_data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join("bench_data");

    fs::read_dir(&bench_data_dir)
        .unwrap_or_else(|error| {
            panic!("failed to read benchmark data directory {}: {error}", bench_data_dir.display())
        })
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry.path().extension().and_then(|ext| ext.to_str()).and_then(|ext| match ext {
                "css" => Some((entry, Syntax::Css)),
                "scss" => Some((entry, Syntax::Scss)),
                "sass" => Some((entry, Syntax::Sass)),
                "less" => Some((entry, Syntax::Less)),
                _ => None,
            })
        })
        .filter(|(entry, ..)| entry.file_type().is_ok_and(|file_type| file_type.is_file()))
        .for_each(|(entry, syntax)| {
            let path = &entry.path();
            let name = entry.file_name();
            let name = &name.to_string_lossy();
            let code = black_box(fs::read_to_string(path).unwrap());

            group.bench_with_input(BenchmarkId::from_parameter(name), &code, |b, code| {
                b.iter(|| {
                    let allocator = Allocator::default();
                    let mut parser = Parser::new(&allocator, code, syntax);
                    black_box(parser.parse::<Stylesheet>().unwrap());
                });
            });
        });
    group.finish();
}

criterion_group!(benches, bench_parser);
criterion_main!(benches);

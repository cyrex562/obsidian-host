use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use obsidian_host::services::MarkdownService;

fn benchmark_simple_markdown(c: &mut Criterion) {
    let markdown =
        "# Hello World\n\nThis is a **simple** markdown document with *some* formatting.";

    c.bench_function("simple_markdown", |b| {
        b.iter(|| MarkdownService::to_html_with_highlighting(black_box(markdown), false))
    });
}

fn benchmark_complex_markdown(c: &mut Criterion) {
    let markdown = r#"# Complex Document

This is a **complex** document with *various* elements.

## Lists

- Item 1
- Item 2
  - Nested item
- Item 3

## Code

```rust
fn main() {
    println!("Hello, world!");
}
```

## Blockquote

> This is a quote
> with multiple lines

## Links

[Link](https://example.com) and ![Image](image.jpg)
"#;

    c.bench_function("complex_markdown", |b| {
        b.iter(|| MarkdownService::to_html_with_highlighting(black_box(markdown), false))
    });
}

fn benchmark_code_highlighting(c: &mut Criterion) {
    let markdown = r#"```rust
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn main() {
    for i in 0..10 {
        println!("fib({}) = {}", i, fibonacci(i));
    }
}
```"#;

    c.bench_function("code_highlighting", |b| {
        b.iter(|| MarkdownService::to_html(black_box(markdown)))
    });
}

fn benchmark_large_document(c: &mut Criterion) {
    let mut markdown = String::new();
    for i in 1..=50 {
        markdown.push_str(&format!("## Section {}\n\n", i));
        markdown.push_str("This is a paragraph with **bold** and *italic* text.\n\n");
        markdown.push_str("- List item 1\n- List item 2\n- List item 3\n\n");
    }

    c.bench_function("large_document_50_sections", |b| {
        b.iter(|| MarkdownService::to_html_with_highlighting(black_box(&markdown), false))
    });
}

fn benchmark_plain_text_extraction(c: &mut Criterion) {
    let markdown = r#"# Title

This is a **complex** document with *various* formatting elements, [links](url), and `code`.

## Section 1

Content here with more **bold** and *italic* text.
"#;

    c.bench_function("plain_text_extraction", |b| {
        b.iter(|| MarkdownService::to_plain_text(black_box(markdown)))
    });
}

fn benchmark_excerpt_generation(c: &mut Criterion) {
    let markdown = "# Long Article\n\nThis is a very long article with lots of content that should be truncated properly when generating an excerpt. It contains multiple paragraphs and various formatting elements.";

    c.bench_function("excerpt_generation", |b| {
        b.iter(|| MarkdownService::get_excerpt(black_box(markdown), 100))
    });
}

fn benchmark_document_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("document_sizes");

    for size in [10, 50, 100, 200].iter() {
        let mut markdown = String::new();
        for i in 1..=*size {
            markdown.push_str(&format!(
                "## Section {}\n\nContent for section {}.\n\n",
                i, i
            ));
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), &markdown, |b, md| {
            b.iter(|| MarkdownService::to_html_with_highlighting(black_box(md), false))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_simple_markdown,
    benchmark_complex_markdown,
    benchmark_code_highlighting,
    benchmark_large_document,
    benchmark_plain_text_extraction,
    benchmark_excerpt_generation,
    benchmark_document_sizes
);

criterion_main!(benches);

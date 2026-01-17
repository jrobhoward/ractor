//! Benchmarks for ractor_shell
//!
//! Run with: `cargo bench -p ractor_shell`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ractor_shell::ShellCommand;

/// Benchmark command parsing for various command types
fn bench_command_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("command_parsing");

    // Simple commands (no arguments)
    let simple_commands = [
        ("actors", "actors"),
        ("registry", "registry"),
        ("nodes", "nodes"),
        ("stats", "stats"),
        ("exit", "exit"),
        ("help", "help"),
    ];

    for (name, cmd) in simple_commands {
        group.bench_with_input(BenchmarkId::new("simple", name), cmd, |b, cmd| {
            b.iter(|| ShellCommand::parse_line(black_box(cmd)))
        });
    }

    // Commands with arguments
    let arg_commands = [
        ("info", "info my_actor"),
        ("stop", "stop my_actor"),
        ("connect", "connect localhost:9000"),
        ("use", "use node_a"),
        ("monitor", "monitor my_actor"),
    ];

    for (name, cmd) in arg_commands {
        group.bench_with_input(BenchmarkId::new("with_arg", name), cmd, |b, cmd| {
            b.iter(|| ShellCommand::parse_line(black_box(cmd)))
        });
    }

    // Commands with multiple arguments
    let multi_arg_commands = [
        ("send_short", r#"send actor {"cmd":"ping"}"#),
        (
            "send_long",
            r#"send my_actor_name {"command":"do_something","data":{"nested":"value"}}"#,
        ),
        ("call", r#"call actor {"get":"state"}"#),
        ("pg_members", "pg members my_group"),
    ];

    for (name, cmd) in multi_arg_commands {
        group.bench_with_input(BenchmarkId::new("multi_arg", name), cmd, |b, cmd| {
            b.iter(|| ShellCommand::parse_line(black_box(cmd)))
        });
    }

    // Alias resolution
    let aliases = [
        ("a_alias", "a"),
        ("r_alias", "r"),
        ("q_alias", "q"),
        ("i_alias", "i actor"),
        ("s_alias", "s actor msg"),
    ];

    for (name, cmd) in aliases {
        group.bench_with_input(BenchmarkId::new("alias", name), cmd, |b, cmd| {
            b.iter(|| ShellCommand::parse_line(black_box(cmd)))
        });
    }

    group.finish();
}

/// Benchmark JSON parsing for messages
fn bench_json_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_parsing");

    let json_inputs = [
        ("simple_object", r#"{"command": "ping"}"#),
        (
            "nested_object",
            r#"{"command": "update", "data": {"id": 1, "name": "test"}}"#,
        ),
        ("array", r#"["item1", "item2", "item3"]"#),
        ("number", "42"),
        ("string", r#""hello world""#),
        ("boolean", "true"),
        ("null", "null"),
        (
            "complex",
            r#"{"actors": [{"name": "a1", "id": 1}, {"name": "a2", "id": 2}], "count": 2}"#,
        ),
    ];

    for (name, json) in json_inputs {
        group.bench_with_input(BenchmarkId::new("parse", name), json, |b, json| {
            b.iter(|| ractor_shell::messages::parse_json_input(black_box(json)))
        });
    }

    group.finish();
}

/// Benchmark error cases (should be fast failures)
fn bench_error_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_cases");

    let error_inputs = [
        ("empty", ""),
        ("whitespace", "   "),
        ("unknown_command", "foobar"),
        ("missing_arg", "info"),
        ("invalid_subcommand", "pg unknown"),
    ];

    for (name, input) in error_inputs {
        group.bench_with_input(BenchmarkId::new("parse_error", name), input, |b, input| {
            b.iter(|| ShellCommand::parse_line(black_box(input)))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_command_parsing,
    bench_json_parsing,
    bench_error_cases
);
criterion_main!(benches);

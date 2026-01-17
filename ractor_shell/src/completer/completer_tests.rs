#![allow(non_snake_case)]

use crate::completer::ShellHelper;
use rustyline::completion::Completer;
use rustyline::Context;

#[test]
fn complete___partial_help_command___suggests_help() {
    let helper = ShellHelper::new();

    let (pos, candidates) = helper
        .complete(
            "he",
            2,
            &Context::new(&rustyline::history::DefaultHistory::new()),
        )
        .unwrap();

    assert_eq!(pos, 0);
    assert!(candidates.iter().any(|c| c.display == "help"));
}

#[test]
fn complete___pg_with_partial_subcommand___suggests_members() {
    let helper = ShellHelper::new();

    let (_pos, candidates) = helper
        .complete(
            "pg m",
            4,
            &Context::new(&rustyline::history::DefaultHistory::new()),
        )
        .unwrap();

    assert!(candidates.iter().any(|c| c.display == "members"));
}

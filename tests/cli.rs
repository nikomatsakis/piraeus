use std::process::Command;

// {test}`requirements.cli.argument_parsing`
#[test]
fn help_is_parsed_before_starting_the_tui() {
    let output = Command::new(env!("CARGO_BIN_EXE_piraeus"))
        .arg("--help")
        .output()
        .expect("run piraeus --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help output");
    assert!(stdout.contains("Usage: piraeus"));
    assert!(stdout.contains("--help"));
}

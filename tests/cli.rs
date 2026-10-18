use std::{
    io::Write,
    process::{Command, Stdio},
};
fn cli(command: &str, input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nua"))
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn stdin_both_directions() {
    let lua = cli("to-luau", "def f() -> number:\n  return 42\n");
    assert!(
        lua.status.success(),
        "{}",
        String::from_utf8_lossy(&lua.stderr)
    );
    assert!(String::from_utf8_lossy(&lua.stdout).contains("local function f() : number"));
    let nua = cli("from-lua", std::str::from_utf8(&lua.stdout).unwrap());
    assert!(nua.status.success());
    assert!(String::from_utf8_lossy(&nua.stdout).contains("def f()"));
}
#[test]
fn errors_are_nonzero_and_do_not_emit_partial_output() {
    let output = cli("to-luau", "def broken():\nreturn 1");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("line 2"));
}
#[test]
fn file_output_and_examples() {
    for name in ["basics", "generics", "methods"] {
        let source = std::fs::read_to_string(format!("examples/{name}.nua")).unwrap();
        let expected = std::fs::read_to_string(format!("examples/{name}.luau")).unwrap();
        assert_eq!(nua::to_luau(&source).unwrap(), expected);
        let back = nua::from_luau(&expected).unwrap();
        assert!(nua::to_luau(&back).is_ok());
    }
    let dir = std::env::temp_dir().join(format!("nua-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("output.luau");
    let output = Command::new(env!("CARGO_BIN_EXE_nua"))
        .args(["to-luau", "examples/basics.nua", "-o"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        std::fs::read_to_string("examples/basics.luau").unwrap()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reading_example() {
    let lua = std::fs::read_to_string("examples/reading.lua").unwrap();
    let expected = std::fs::read_to_string("examples/reading.nua").unwrap();
    assert_eq!(nua::from_luau(&lua).unwrap(), expected);
}

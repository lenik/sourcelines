use std::process::Command;

#[test]
fn test_python_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_sourcelines"))
        .arg("tests/testdata/simple.py")
        .output()
        .expect("failed to run sourcelines");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("python"));
    assert!(stdout.contains("simple.py"));
    // 2 code lines, 3 comment lines, 2 empty lines
    assert!(stdout.contains("2 ")); // actual_loc
    assert!(stdout.contains("7 ")); // raw_loc
}

#[test]
fn test_c_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_sourcelines"))
        .arg("tests/testdata/simple.c")
        .output()
        .expect("failed to run sourcelines");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("c"));
    assert!(stdout.contains("simple.c"));
    assert!(stdout.contains("3 ")); // actual_loc
    assert!(stdout.contains("8 ")); // raw_loc
}

#[test]
fn test_shell_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_sourcelines"))
        .arg("tests/testdata/simple.sh")
        .output()
        .expect("failed to run sourcelines");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("shell"));
    assert!(stdout.contains("simple.sh"));
    assert!(stdout.contains("2 ")); // actual_loc
    assert!(stdout.contains("6 ")); // raw_loc
}

#[test]
fn test_txt_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_sourcelines"))
        .arg("tests/testdata/simple.txt")
        .output()
        .expect("failed to run sourcelines");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("text"));
    assert!(stdout.contains("simple.txt"));
    assert!(stdout.contains("3 ")); // actual_loc
    assert!(stdout.contains("5 ")); // raw_loc
}

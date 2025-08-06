use std::process::Command;
use std::fs;

fn main() {
    // Convert markdown man page to roff if pandoc is available
    if Command::new("which").arg("pandoc").output().map(|o| o.status.success()).unwrap_or(false) {
        let _ = Command::new("pandoc")
            .args(["-s", "-t", "man", "wcc.1.md", "-o", "wcc.1"])
            .status();
    } else {
        // fallback: copy markdown as .1 for packaging, but warn
        let _ = fs::copy("wcc.1.md", "wcc.1");
        println!("cargo:warning=Pandoc not found, wcc.1 is not a real man page");
    }
}

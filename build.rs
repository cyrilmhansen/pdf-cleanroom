use std::process::Command;

fn git_output(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn git_dirty() -> String {
    match Command::new("git").args(["diff", "--quiet"]).status() {
        Ok(status) if status.success() => "false".to_string(),
        Ok(_) => "true".to_string(),
        Err(_) => "unknown".to_string(),
    }
}

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-env-changed=PROFILE");

    println!(
        "cargo:rustc-env=PDF_CLEANROOM_GIT_HASH={}",
        git_output(&["rev-parse", "--short", "HEAD"])
    );
    println!(
        "cargo:rustc-env=PDF_CLEANROOM_GIT_BRANCH={}",
        git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
    );
    println!("cargo:rustc-env=PDF_CLEANROOM_GIT_DIRTY={}", git_dirty());
}

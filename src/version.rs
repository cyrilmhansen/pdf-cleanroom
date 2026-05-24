/// Build and source-control metadata for the running binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: &'static str,
    pub commit: &'static str,
    pub branch: &'static str,
    pub dirty: &'static str,
    pub profile: &'static str,
}

/// Return compile-time version metadata.
pub const fn build_info() -> BuildInfo {
    BuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        commit: env!("PDF_CLEANROOM_GIT_HASH"),
        branch: env!("PDF_CLEANROOM_GIT_BRANCH"),
        dirty: env!("PDF_CLEANROOM_GIT_DIRTY"),
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    }
}

/// Human-readable multi-line version output for diagnostics.
pub fn version_string() -> String {
    let info = build_info();
    format!(
        "pdf-cleanroom {}\ncommit: {}\nbranch: {}\ndirty: {}\nprofile: {}",
        info.version, info.commit, info.branch, info.dirty, info.profile
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_output_includes_crate_version() {
        let output = version_string();
        assert!(
            output.contains(env!("CARGO_PKG_VERSION")),
            "version output should include crate version: {output}"
        );
        assert!(output.contains("commit: "));
        assert!(output.contains("branch: "));
        assert!(output.contains("dirty: "));
        assert!(output.contains("profile: "));
    }
}

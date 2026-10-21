//! The preset file and the copy the crate carries must not drift apart.
//!
//! The tuning surface is `assets/presets.toml`, one file in the repository.
//! A published crate cannot read a file from outside itself, so the crate
//! carries a copy of it, and that copy is what crates.io users get. If the
//! two ever differ, the published crate would ship different presets from
//! the ones the repository describes — so the difference is an error, not a
//! detail.
//!
//! In a published crate there is no `assets/` directory, so the test has
//! nothing to compare and passes.

use std::path::PathBuf;

/// The tuning surface, in the repository.
const PRESETS_IN_REPO: &str = "../../assets/presets.toml";

/// The copy the crate carries.
const PRESETS_IN_CRATE: &str = "presets.toml";

#[test]
fn the_crates_copy_of_the_presets_is_current() {
    let repo = PathBuf::from(PRESETS_IN_REPO);
    if !repo.exists() {
        // A published crate: there is nothing to compare it with.
        return;
    }

    let in_crate = PathBuf::from(PRESETS_IN_CRATE);
    assert!(
        in_crate.exists(),
        "the crate must carry a copy of the presets, so that it can be published"
    );

    let repo_text = std::fs::read_to_string(&repo).expect("cannot read the preset file");
    let crate_text = std::fs::read_to_string(&in_crate).expect("cannot read the preset copy");

    assert_eq!(
        repo_text, crate_text,
        "the crate's copy of the presets is out of date; copy it over with:\n\
         cp {PRESETS_IN_REPO} {PRESETS_IN_CRATE}"
    );
}

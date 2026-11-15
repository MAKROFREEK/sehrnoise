//! The user presets: the curves the user saves themselves (T7.4).
//!
//! The built-in table lives in `assets/presets.toml` and is compiled into the
//! core at build time; the built-in table is never touched by this module.
//! The user presets are runtime data: a Save button in the panel writes the
//! current state of the panel — the eight sliders, the warp and the width
//! choices — into a JSON file, and the presets read back from that file
//! appear at the end of the panel's preset dropdown.
//!
//! The file lives in the user's configuration directory
//! (`~/.config/sehrnoise/user-presets.json` on Linux, `Application Support`
//! on macOS, `AppData` on Windows — the architecture's §10 decision, brought
//! forward from v1.1 because T7.11 froze the plugin target and the
//! standalone application has no host to keep presets for it).
//!
//! ## The format
//!
//! The architecture's example carries `seed` and `drift` fields; both are
//! gone. A saved preset is a snapshot of concrete gains: the Randomize
//! curve can be saved (the question T7.4 posed), because the rolled gains
//! sit in the sliders and are saved as they are — re-selecting the saved
//! preset plays the same curve again, it does not re-roll. The drift is a
//! session-only stage (T7.8) and not part of a preset; saving while the
//! drift runs saves the set gains, without the drift's current offsets.
//! The master gain and the mute are not saved either — the ticket asks for
//! the eight sliders, the warp and the width, nothing else.
//!
//! ## Failure behaviour
//!
//! A missing file is not an error (first run). A file that cannot be read
//! or parsed is preserved under a `.bak` name before anything else happens,
//! so a later save cannot silently destroy it. A save that fails leaves the
//! in-memory list untouched: the panel and the file never disagree.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sehrnoise_core::{NUM_BANDS, Warp, Width};

/// How many characters a preset name may hold.
///
/// A saved name longer than this is truncated on commit. The dropdown
/// additionally truncates what it displays (see `ui.rs`), so that a long
/// name cannot stretch the pinned panel wider than it is.
const MAX_NAME_CHARS: usize = 32;

/// One saved curve over the eight bands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPreset {
    /// The name shown in the dropdown, as the user typed it.
    pub name: String,
    /// The gain of each band, in dB.
    pub bands_db: [f32; NUM_BANDS],
    /// Spectral tilt. Stored as its short name ("Reset", "Slower").
    #[serde(with = "warp_serde")]
    pub warp: Warp,
    /// Stereo width. Stored as its short name ("Mono", "Normal", "Wide").
    #[serde(with = "width_serde")]
    pub width: Width,
}

/// The file's top level: a version, so a future format can be recognised.
#[derive(Serialize, Deserialize)]
struct UserPresetFile {
    version: u32,
    presets: Vec<UserPreset>,
}

/// The version of the file format this module writes and reads.
const FORMAT_VERSION: u32 = 1;

mod warp_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use sehrnoise_core::Warp;

    pub fn serialize<S: Serializer>(warp: &Warp, serializer: S) -> Result<S::Ok, S::Error> {
        warp.as_str().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Warp, D::Error> {
        let text = String::deserialize(deserializer)?;
        Warp::from_str(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown warp: {text}")))
    }
}

mod width_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use sehrnoise_core::Width;

    pub fn serialize<S: Serializer>(width: &Width, serializer: S) -> Result<S::Ok, S::Error> {
        width.as_str().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Width, D::Error> {
        let text = String::deserialize(deserializer)?;
        Width::from_str(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown width: {text}")))
    }
}

/// The user presets the panel knows about, plus the state of the Save prompt.
///
/// The store is owned by the editor (`SehrNoiseApp`) and handed to
/// [`crate::ui::draw`], which reads the list for the dropdown and drives the
/// Save prompt through it. It is user-interface state: the audio thread
/// never sees it — a selected user preset reaches the sound through the
/// band, warp and width parameters, which both threads already share.
pub struct UserPresetStore {
    /// Where the file lives. `None` when no configuration directory could
    /// be found: the list still works in memory, but saving fails.
    path: Option<PathBuf>,
    /// The presets, in the order the file holds them.
    presets: Vec<UserPreset>,
    /// Whether the name prompt is open.
    prompt_open: bool,
    /// What the user has typed into the name prompt so far.
    buffer: String,
    /// The problem the last save ran into, shown in the prompt row.
    error: Option<String>,
}

impl UserPresetStore {
    /// Load the user presets from the default location.
    ///
    /// A missing file is a fresh start, not an error. Anything unreadable is
    /// preserved as `user-presets.json.bak` and the list starts empty, so
    /// the next save cannot destroy data it could not understand.
    pub fn load() -> Self {
        match default_path() {
            Some(path) => Self::at(path),
            None => Self {
                path: None,
                presets: Vec::new(),
                prompt_open: false,
                buffer: String::new(),
                error: None,
            },
        }
    }

    /// Load from one explicit path (also how the tests get a store without
    /// touching the real configuration directory).
    pub fn at(path: PathBuf) -> Self {
        let presets = Self::read(&path);
        Self {
            path: Some(path),
            presets,
            prompt_open: false,
            buffer: String::new(),
            error: None,
        }
    }

    /// A store with nothing in it and nowhere to save (tests, and any
    /// environment without a configuration directory).
    pub fn detached() -> Self {
        Self {
            path: None,
            presets: Vec::new(),
            prompt_open: false,
            buffer: String::new(),
            error: None,
        }
    }

    /// Read the file, with the `.bak` recovery described on [`Self::load`].
    fn read(path: &std::path::Path) -> Vec<UserPreset> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(e) => {
                eprintln!("sehrnoise: cannot read {}: {e}", path.display());
                return Vec::new();
            }
        };
        match parse(&text) {
            Ok(presets) => presets,
            Err(e) => {
                eprintln!(
                    "sehrnoise: {} is not a usable user preset file ({e}); \
                     keeping a copy as {}.bak",
                    path.display(),
                    path.display()
                );
                let _ = std::fs::copy(path, path.with_extension("json.bak"));
                Vec::new()
            }
        }
    }

    /// The saved presets, in file order.
    pub fn presets(&self) -> &[UserPreset] {
        &self.presets
    }

    /// The preset at a dropdown index past the built-in table, if there is one.
    pub fn get(&self, index: usize) -> Option<&UserPreset> {
        self.presets.get(index)
    }

    /// Whether the list is empty (the dropdown shows no separator then).
    pub fn is_empty(&self) -> bool {
        self.presets.is_empty()
    }

    /// Add a preset, or replace the one of the same name.
    ///
    /// The list only changes if the file write succeeds, so the panel and
    /// the file never disagree. Returns the index of the saved preset.
    pub fn commit(&mut self, mut preset: UserPreset) -> Result<usize, String> {
        let name = truncate(&preset.name, MAX_NAME_CHARS);
        if name.trim().is_empty() {
            return Err("the name is empty".to_string());
        }
        preset.name = name;

        let path = self
            .path
            .as_ref()
            .ok_or_else(|| "no configuration directory found".to_string())?;

        // Build the new list first; the old one stays until the write
        // succeeds.
        let mut next = self.presets.clone();
        let index = match next.iter().position(|p| p.name == preset.name) {
            Some(index) => {
                next[index] = preset.clone();
                index
            }
            None => {
                next.push(preset.clone());
                next.len() - 1
            }
        };

        let file = UserPresetFile {
            version: FORMAT_VERSION,
            presets: next.clone(),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("cannot write the presets: {e}"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(path, text + "\n")
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;

        self.presets = next;
        Ok(index)
    }

    /// Whether the name prompt is open.
    pub fn prompt_open(&self) -> bool {
        self.prompt_open
    }

    /// Open the name prompt: empty buffer, no error.
    pub fn open_prompt(&mut self) {
        self.prompt_open = true;
        self.buffer.clear();
        self.error = None;
    }

    /// Close the name prompt without saving.
    pub fn cancel_prompt(&mut self) {
        self.prompt_open = false;
        self.error = None;
    }

    /// What the user has typed into the name prompt.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// The prompt's text buffer, for the text edit to drive.
    pub fn buffer_mut(&mut self) -> &mut String {
        &mut self.buffer
    }

    /// The problem the last save ran into, if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Record a save problem, to show in the prompt row.
    pub fn set_error(&mut self, error: &str) {
        self.error = Some(error.to_string());
    }
}

/// Parse and validate the file's text.
///
/// Strict: one bad entry makes the whole file unusable, and the `.bak`
/// copy keeps every entry for the user to sort out by hand. A `version`
/// this module does not know is treated the same way — better a `.bak`
/// than a silent overwrite by an older format.
fn parse(text: &str) -> Result<Vec<UserPreset>, String> {
    let file: UserPresetFile =
        serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
    if file.version != FORMAT_VERSION {
        return Err(format!(
            "the file says version {}, this build reads version {FORMAT_VERSION}",
            file.version
        ));
    }
    Ok(file.presets)
}

/// Truncate a string to at most `limit` characters, whole characters only.
fn truncate(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// Where user presets live, from the environment.
///
/// Linux reads `XDG_CONFIG_HOME` (absolute paths only, as the XDG
/// specification requires) and falls back to `$HOME/.config`; macOS uses
/// `~/Library/Application Support`; Windows uses `%APPDATA%`.
pub fn default_path() -> Option<PathBuf> {
    let dir = match std::env::consts::OS {
        "linux" => linux_config_dir(
            std::env::var("XDG_CONFIG_HOME").ok(),
            std::env::var("HOME").ok(),
        ),
        "macos" => std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join("Library/Application Support")),
        "windows" => std::env::var("APPDATA").ok().map(PathBuf::from),
        _ => None,
    }?;
    Some(path_in(dir))
}

/// The file's path inside a configuration directory.
pub fn path_in(config_dir: PathBuf) -> PathBuf {
    config_dir.join("sehrnoise").join("user-presets.json")
}

/// The Linux configuration directory, as a pure function of the two
/// environment variables (so the XDG rules are testable).
fn linux_config_dir(xdg: Option<String>, home: Option<String>) -> Option<PathBuf> {
    if let Some(dir) = xdg {
        let dir = PathBuf::from(dir);
        // The XDG specification: a relative XDG_CONFIG_HOME is ignored.
        if dir.is_absolute() {
            return Some(dir);
        }
    }
    home.map(|home| PathBuf::from(home).join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique scratch directory, so parallel tests never share a file.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("sehrnoise-userpresets-tests")
            .join(format!("{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("user-presets.json")
    }

    fn a_preset(name: &str) -> UserPreset {
        UserPreset {
            name: name.to_string(),
            bands_db: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            warp: Warp::Slower,
            width: Width::Wide,
        }
    }

    #[test]
    fn the_linux_config_directory_follows_the_xdg_rules() {
        // An absolute XDG_CONFIG_HOME wins.
        assert_eq!(
            linux_config_dir(Some("/xdg".to_string()), Some("/home/u".to_string())),
            Some(PathBuf::from("/xdg"))
        );
        // A relative one is ignored, as the specification requires.
        assert_eq!(
            linux_config_dir(Some("relative".to_string()), Some("/home/u".to_string())),
            Some(PathBuf::from("/home/u/.config"))
        );
        // No XDG, no home: no directory.
        assert_eq!(linux_config_dir(None, None), None);
    }

    #[test]
    fn the_file_lives_in_the_sehrnoise_directory() {
        assert_eq!(
            path_in(PathBuf::from("/cfg")),
            PathBuf::from("/cfg/sehrnoise/user-presets.json")
        );
    }

    #[test]
    fn a_saved_preset_survives_a_reload() {
        let path = scratch("roundtrip");
        let mut store = UserPresetStore::at(path.clone());
        let index = store.commit(a_preset("My Curve")).unwrap();
        assert_eq!(index, 0);

        let reloaded = UserPresetStore::at(path);
        assert_eq!(reloaded.presets().len(), 1);
        let saved = &reloaded.presets()[0];
        assert_eq!(saved.name, "My Curve");
        assert_eq!(saved.bands_db, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        assert_eq!(saved.warp, Warp::Slower);
        assert_eq!(saved.width, Width::Wide);
    }

    #[test]
    fn the_same_name_replaces_the_old_preset() {
        let path = scratch("upsert");
        let mut store = UserPresetStore::at(path.clone());
        store.commit(a_preset("Same")).unwrap();
        let mut second = a_preset("Same");
        second.bands_db = [8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        let index = store.commit(second).unwrap();

        // Replaced, not appended.
        assert_eq!(index, 0);
        assert_eq!(store.presets().len(), 1);
        assert_eq!(store.presets()[0].bands_db[0], 8.0);
    }

    #[test]
    fn a_long_name_is_truncated_whole_characters() {
        let path = scratch("truncate");
        let mut store = UserPresetStore::at(path);
        let mut preset = a_preset("x");
        preset.name = "n".repeat(100);
        store.commit(preset).unwrap();
        assert_eq!(store.presets()[0].name.len(), MAX_NAME_CHARS);
    }

    #[test]
    fn an_unreadable_file_is_preserved_as_a_bak_copy() {
        let path = scratch("corrupt");
        std::fs::write(&path, "{ not json").unwrap();
        let store = UserPresetStore::at(path.clone());

        assert!(store.is_empty());
        let bak = path.with_extension("json.bak");
        assert_eq!(
            std::fs::read_to_string(&bak).unwrap(),
            "{ not json",
            "the .bak copy lost the original bytes"
        );
    }

    #[test]
    fn a_future_format_is_not_overwritten_by_this_one() {
        let path = scratch("future");
        std::fs::write(&path, r#"{ "version": 99, "presets": [] }"#).unwrap();
        let store = UserPresetStore::at(path.clone());
        assert!(store.is_empty());
        assert!(path.with_extension("json.bak").exists());
    }

    #[test]
    fn a_missing_file_is_a_fresh_start_not_an_error() {
        let store = UserPresetStore::at(scratch("missing"));
        assert!(store.is_empty());
    }

    #[test]
    fn a_failed_commit_leaves_the_list_untouched() {
        let mut store = UserPresetStore::detached();
        let result = store.commit(a_preset("Anywhere"));
        assert!(result.is_err(), "there is nowhere to write");
        assert!(store.is_empty());
    }

    #[test]
    fn an_empty_name_is_refused() {
        let path = scratch("empty-name");
        let mut store = UserPresetStore::at(path);
        let mut preset = a_preset("   ");
        preset.name = "  ".to_string();
        assert!(store.commit(preset).is_err());
        assert!(store.is_empty());
    }
}

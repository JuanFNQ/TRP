//! One key of the options file read or kept, with everything else in the file left as it was.

use bevy::log::warn;
use serde_json::Value;
use std::path::Path;

/// The file's content, where it is there and reads as JSON.
pub fn read(file: &Path) -> Option<Value> {
    std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str(&text).ok())
}

/// Keeps `value` under `key`, or takes the key out for `None`, with all else in the file as it
/// was. A file that holds something other than a JSON object is left alone. `what` names the
/// thing in the warning written when the file cannot be written.
pub fn keep(file: &Path, key: &str, value: Option<Value>, what: &str) {
    let mut all = read(file).unwrap_or_else(|| serde_json::json!({}));
    let Some(map) = all.as_object_mut() else { return };
    match value {
        Some(value) => {
            map.insert(key.to_owned(), value);
        }
        None => {
            map.remove(key);
        }
    }
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    if let Err(e) = std::fs::write(file, serde_json::to_string_pretty(&all).unwrap_or_default()) {
        warn!("{what} could not be kept: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_kept_and_taken_out_with_the_rest_left_alone() {
        let file = std::env::temp_dir().join(format!("trp_options_store_{}.json", std::process::id()));
        std::fs::write(&file, r#"{ "graphics": { "vsync": true } }"#).unwrap();
        keep(&file, "voice", Some(serde_json::json!("Usec_1")), "the voice");
        let all = read(&file).unwrap();
        assert_eq!(all["voice"], "Usec_1");
        assert_eq!(all["graphics"]["vsync"], true);
        keep(&file, "voice", None, "the voice");
        let all = read(&file).unwrap();
        assert!(all.get("voice").is_none());
        assert_eq!(all["graphics"]["vsync"], true);
        // A file that is not an object is left as it is.
        std::fs::write(&file, "[1, 2]").unwrap();
        keep(&file, "voice", Some(serde_json::json!("x")), "the voice");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[1, 2]");
        let _ = std::fs::remove_file(&file);
    }
}

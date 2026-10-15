use std::ffi::OsStr;
use std::path::Path;

/// Checks if a file has a given extension
// https://stackoverflow.com/questions/72392835/check-if-a-file-is-of-a-given-type
pub trait FileExtension {
    fn has_extension<S: AsRef<str>>(&self, extensions: &[S]) -> bool;
}

impl<P: AsRef<Path>> FileExtension for P {
    fn has_extension<S: AsRef<str>>(&self, extensions: &[S]) -> bool {
        if let Some(extension) = self.as_ref().extension().and_then(OsStr::to_str) {
            return extensions
                .iter()
                .any(|x| x.as_ref().eq_ignore_ascii_case(extension));
        }

        false
    }
}

pub fn is_audio_file(path: &str) -> bool {
    Path::new(path).has_extension(&[
        "mp3", "ogg", "flac", "m4a", "wav", "aac", "opus", "ape", "wma", "mpc", "aiff", "dff",
        "mp2", "mka",
    ])
}

use mpd::{Client, Song};

#[derive(Debug)]
#[derive(Clone )]
pub struct RSong {
    pub file: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub duration: Option<u32>,
    pub last_mod: Option<String>,
    pub name: Option<String>,
    pub place: Option<String>,
    pub range: Option<String>,
    pub tags: Vec<(String, String)>,
}

impl RSong {
    pub fn new(c: &mut Client, filename: String) -> Self {
        let mut s = RSong {
            file: filename.clone(),
            artist: None,
            title: None,
            duration: None,
            last_mod: None,
            name: None,
            place: None,
            range: None,
            tags: vec![],
        };

        // Dummy song

        let song = Song {
            file: filename.clone(),
            artist: None,
            title: None,
            duration: None,
            last_mod: None,
            name: None,
            place: None,
            range: None,
            tags: vec![("".to_string(), "".to_string())],
        };

        for (k, v) in (c.readcomments(song).unwrap()).flatten() {
            if k.to_lowercase().contains("artist") {
                s.artist = Some(v);
            } else if k.to_lowercase().contains("title") {
                s.title = Some(v);
            } else if k.to_lowercase().contains("duration") {
                s.duration = Some(v.parse::<u32>().unwrap());
            } else if k.to_lowercase().contains("lastmod") {
                s.last_mod = Some(v);
            } else if k.to_lowercase().contains("name") {
                s.name = Some(v);
            } else if k.to_lowercase().contains("place") {
                s.place = Some(v);
            } else if k.to_lowercase().contains("range") {
                s.range = Some(v);
            } else {
                s.tags.push((k, v));
            }
        }

        s
    }
}

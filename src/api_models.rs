use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct SpotifyTokenResponse {
    pub access_token: String,
    token_type: String,
    scope: String,
    expires_in: u64,
    pub refresh_token: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyUserProfile {
    pub display_name: String,
    pub id: String,
    pub images: Option<Vec<SpotifyImageObject>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyImageObject {
    pub url: String,
    pub height: Option<u32>,
    pub width: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyPlaylistItem {
    pub id: String,
    pub name: String,
    pub images: Option<Vec<SpotifyImageObject>>,
    pub description: Option<String>,
    pub uri: String, // Add owner, public, collaborative, tracks url etc. if needed
}

// For the /me/playlists endpoint top-level response
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyPlaylistsResponse {
    pub items: Vec<SpotifyPlaylistItem>,
    pub href: String,
    pub limit: u32,
    pub next: Option<String>,
    pub offset: u32,
    pub previous: Option<String>,
    pub total: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyPlaylistTrackResponse {
    pub items: Vec<PlaylistItemTrackWrapper>,
    //pub href: String,
    pub limit: u32,
    pub next: Option<String>,
    pub offset: u32,
    pub previous: Option<String>,
    pub total: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlaylistItemTrackWrapper {
    pub track: Option<SpotifyTrackItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyTrackItem {
    pub id: Option<String>,
    pub uri: String,
    pub name: String,
    pub artists: Vec<SpotifyArtistSimple>,
    pub album: Option<SpotifyTrackAlbumSimple>,
    pub duration_ms: Option<u32>,
    pub explicit: Option<bool>,
    pub audio_features: Option<SpotifyAudioFeatures>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyTrackAlbumSimple {
    pub id: Option<String>,
    pub name: String,
    pub images: Option<Vec<SpotifyImageObject>>,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyArtistSimple {
    pub id: Option<String>,
    pub name: String,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NewPlaylistDetails {
    pub id: String,
    pub name: String,
    pub external_url: String, // The web URL to the new playlist
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpotifyAudioFeatures {
    pub id: String,
    pub acousticness: f64,
    pub danceability: f64,
    pub duration_ms: u32,
    pub energy: f64,
    pub instrumentalness: f64,
    pub key: i32,
    pub liveness: f64,
    pub loudness: f64,
    pub mode: i32,
    pub speechiness: f64,
    pub tempo: f64,
    pub time_signature: i32,
    pub valence: f64,
    #[serde(rename = "type")]
    pub track_type: String,
    pub uri: String,
    pub track_href: String,
    pub analysis_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AudioFeaturesResponse {
    pub audio_features: Vec<Option<SpotifyAudioFeatures>>,
}

impl SpotifyAudioFeatures {
    /// Convert to JSON string for storage in Neo4j
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    
    /// Create from JSON string stored in Neo4j
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
    
    /// Get a human-readable summary of the audio features
    pub fn summary(&self) -> String {
        format!(
            "Tempo: {:.1} BPM, Energy: {:.1}/10, Danceability: {:.1}/10, Valence: {:.1}/10",
            self.tempo,
            self.energy * 10.0,
            self.danceability * 10.0,
            self.valence * 10.0
        )
    }
}

impl SpotifyTrackItem {
    /// Get the primary artist name
    pub fn primary_artist(&self) -> String {
        self.artists.first()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string())
    }
    
    /// Get all artist names joined
    pub fn all_artists(&self) -> String {
        self.artists.iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<&str>>()
            .join(", ")
    }
    
    /// Get a display string for the song
    pub fn display_name(&self) -> String {
        format!("{} - {}", self.name, self.primary_artist())
    }
    
    /// Check if track has audio features
    pub fn has_audio_features(&self) -> bool {
        self.audio_features.is_some()
    }
}
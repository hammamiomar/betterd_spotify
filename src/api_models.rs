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
    pub id: String,
    pub uri: String,
    pub name: String,
    pub artists: Vec<SpotifyArtistSimple>,
    pub album: Option<SpotifyTrackAlbumSimple>,
    pub duration_ms: Option<u32>,
    pub explicit: Option<bool>,
    pub audio_features: Option<LLMSongFeatures>,
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
pub struct LLMSongFeatures {
    pub song_id: String,
    pub metadata_confidence: MetadataConfidence,
    
    // Flat structures for easy Neo4j node creation
    pub genres: Vec<GenreFeature>,           // -> Genre nodes
    pub cultural_origins: Vec<CulturalFeature>, // -> Culture nodes  
    pub primary_language: Option<String>,     // -> Language node
    pub instruments: Vec<String>,            // -> Instrument nodes
    pub energy_level: Option<String>,        // -> EnergyLevel node
    pub valence: Option<String>,             // -> Valence node
    pub production_style: Option<String>,    // -> ProductionStyle node
    pub vocal_presence: Option<String>,      // -> VocalStyle node
    pub user_facing_tags: Vec<UserTag>,     // -> UserTag nodes
    
    // Simple properties (stored as Song node properties)
    pub rhythmic_feel: Option<String>,
    pub density: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MetadataConfidence {
    pub level: String,  // "Very High", "High", "Medium", "Low", "Very Low"
    pub explanation: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GenreFeature {
    pub name: String,
    pub confidence: f64,
    pub is_primary: bool,    // Mark primary genre for special handling
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CulturalFeature {
    pub name: String,
    pub confidence: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UserTag {
    pub tag: String,
    pub confidence: f64,
}

// Keep this as an alias for backward compatibility during transition
pub type SpotifyAudioFeatures = LLMSongFeatures;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AudioFeaturesResponse {
    pub audio_features: Vec<Option<SpotifyAudioFeatures>>,
}

impl LLMSongFeatures {
    /// Convert to JSON string for storage in Neo4j
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    
    /// Create from JSON string stored in Neo4j
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
    
    /// Get a human-readable summary of the LLM-generated features
    pub fn summary(&self) -> String {
        let primary_genre = self.genres.iter()
            .find(|g| g.is_primary)
            .map(|g| g.name.as_str())
            .unwrap_or("Unknown");
        
        let energy = self.energy_level.as_deref().unwrap_or("Unknown");
        let valence = self.valence.as_deref().unwrap_or("Unknown");
        let language = self.primary_language.as_deref().unwrap_or("Unknown");
        
        format!(
            "Genre: {}, Energy: {}, Valence: {}, Language: {}, Confidence: {}",
            primary_genre, energy, valence, language, self.metadata_confidence.level
        )
    }
    
    /// Get the primary genre name
    pub fn primary_genre(&self) -> Option<&str> {
        self.genres.iter()
            .find(|g| g.is_primary)
            .map(|g| g.name.as_str())
    }
    
    /// Get all genre names for display
    pub fn all_genres(&self) -> Vec<&str> {
        self.genres.iter().map(|g| g.name.as_str()).collect()
    }
    
    /// Check if song has high confidence metadata
    pub fn has_high_confidence(&self) -> bool {
        matches!(self.metadata_confidence.level.as_str(), "Very High" | "High")
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

/// Playlist imported in the database with enrichment status
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ImportedPlaylist {
    pub id: String,
    pub name: String,
    pub description: String,
    pub song_count: i64,
    pub enriched_count: i64,
    pub has_enrichment: bool,
    pub fully_enriched: bool,
}

impl ImportedPlaylist {
    /// Get enrichment status as a user-friendly string
    pub fn enrichment_status(&self) -> String {
        if self.song_count == 0 {
            "Empty".to_string()
        } else if self.fully_enriched {
            "Fully Enriched".to_string()
        } else if self.has_enrichment {
            format!("Partial ({}/{})", self.enriched_count, self.song_count)
        } else {
            "Not Enriched".to_string()
        }
    }
    
    /// Get enrichment percentage
    pub fn enrichment_percentage(&self) -> f64 {
        if self.song_count == 0 {
            0.0
        } else {
            (self.enriched_count as f64 / self.song_count as f64) * 100.0
        }
    }
    
    /// Check if playlist can be enriched (has songs and not fully enriched)
    pub fn can_be_enriched(&self) -> bool {
        self.song_count > 0 && !self.fully_enriched
    }
}

/// Node types available for playlist filtering
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NodeTypeInfo {
    pub node_type: String,      // "Genre", "Language", "EnergyLevel", etc.
    pub available_values: Vec<NodeValue>,
}

/// Individual node value with count of songs
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)] 
pub struct NodeValue {
    pub value: String,          // The actual value like "Electronic", "English", etc.
    pub song_count: i64,        // How many songs have this value
    pub confidence_avg: Option<f64>, // Average confidence if applicable
}

/// Playlist generation request
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlaylistGenerationRequest {
    pub name: String,
    pub description: Option<String>,
    pub filters: Vec<PlaylistFilter>,
    pub max_songs: u32,
    pub shuffle: bool,
}

/// Individual filter for playlist generation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlaylistFilter {
    pub node_type: String,      // "Genre", "Language", etc.
    pub value: String,          // "Electronic", "English", etc.
    pub min_confidence: Option<f64>, // Optional confidence threshold
}

/// Result of playlist generation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneratedPlaylistResult {
    pub spotify_playlist_id: String,
    pub name: String,
    pub song_count: i64,
    pub applied_filters: Vec<String>,
    pub external_url: String,
}
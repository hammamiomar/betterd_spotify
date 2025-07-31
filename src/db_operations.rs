use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::{
    api::{get_spotify_playlist, get_spotify_playlist_tracks_all},
    auth::helpers::require_auth,
    server::AppState,
};

#[cfg(feature = "server")]
use neo4rs::query;

#[cfg(feature = "server")]
use tracing;

/// Import a selected playlist into the graph database
#[server(ImportPlaylistToDb)]
pub async fn import_playlist_to_db(playlist_id: String) -> Result<String, ServerFnError> {
    let user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    tracing::info!("Starting import of playlist {} for user {}", playlist_id, user.spotify_id);
    
    // 1. Get playlist details
    let playlist = get_spotify_playlist(playlist_id.clone()).await?;
    tracing::info!("Importing playlist: {} - {}", playlist.name, playlist.id);
    
    // 2. Get all tracks from the playlist
    let tracks = get_spotify_playlist_tracks_all(playlist_id.clone()).await?;
    tracing::info!("Found {} tracks in playlist", tracks.len());
    
    if tracks.is_empty() {
        return Ok(format!("Playlist '{}' is empty", playlist.name));
    }
    
    // 3. Store playlist in database
    let mut playlist_query = query(
        "MATCH (u:User {spotify_id: $user_id})
         MERGE (p:Playlist {spotify_id: $playlist_id})
         SET p.name = $name,
             p.description = $description,
             p.total_tracks = $total_tracks,
             p.image_url = $image_url,
             p.imported_at = datetime()
         MERGE (u)-[:IMPORTED]->(p)
         RETURN p.spotify_id as id"
    );
    playlist_query = playlist_query
        .param("user_id", user.spotify_id.clone())
        .param("playlist_id", playlist.id.clone())
        .param("name", playlist.name.clone())
        .param("description", playlist.description.unwrap_or_default())
        .param("total_tracks", tracks.len() as i64)
        .param("image_url", playlist.images.as_ref()
            .and_then(|imgs| imgs.first())
            .map(|img| img.url.clone())
            .unwrap_or_default());
    
    if let Err(e) = app_state.db.run(playlist_query).await {
        return Err(ServerFnError::ServerError(format!("Failed to store playlist: {}", e)));
    }
    
    // 4. Process tracks in batches of 50
    let mut imported_tracks = 0;
    let mut imported_artists = 0;
    
    for (batch_idx, track_batch) in tracks.chunks(50).enumerate() {
        tracing::info!("Processing batch {} of tracks", batch_idx + 1);
        
        // Store tracks
        for track in track_batch {
            if let Some(track_id) = &track.id {
                // Store track
                let mut track_query = query(
                    "MERGE (t:Track {spotify_id: $track_id})
                     SET t.name = $name,
                         t.uri = $uri,
                         t.duration_ms = $duration_ms,
                         t.explicit = $explicit
                     RETURN t.spotify_id as id"
                );
                track_query = track_query
                    .param("track_id", track_id.clone())
                    .param("name", track.name.clone())
                    .param("uri", track.uri.clone())
                    .param("duration_ms", track.duration_ms.unwrap_or(0) as i64)
                    .param("explicit", track.explicit.unwrap_or(false));
                
                if let Err(e) = app_state.db.run(track_query).await {
                    return Err(ServerFnError::ServerError(format!("Failed to store track: {}", e)));
                }
                
                // Connect playlist to track
                let mut playlist_track_query = query(
                    "MATCH (p:Playlist {spotify_id: $playlist_id})
                     MATCH (t:Track {spotify_id: $track_id})
                     MERGE (p)-[:CONTAINS]->(t)"
                );
                playlist_track_query = playlist_track_query
                    .param("playlist_id", playlist.id.clone())
                    .param("track_id", track_id.clone());
                
                if let Err(e) = app_state.db.run(playlist_track_query).await {
                    return Err(ServerFnError::ServerError(format!("Failed to connect playlist to track: {}", e)));
                }
                
                imported_tracks += 1;
                
                // Store artists for this track
                for artist in &track.artists {
                    if let Some(artist_id) = &artist.id {
                        // Store artist
                        let mut artist_query = query(
                            "MERGE (a:Artist {spotify_id: $artist_id})
                             SET a.name = $name
                             RETURN a.spotify_id as id"
                        );
                        artist_query = artist_query
                            .param("artist_id", artist_id.clone())
                            .param("name", artist.name.clone());
                        
                        if let Err(e) = app_state.db.run(artist_query).await {
                            return Err(ServerFnError::ServerError(format!("Failed to store artist: {}", e)));
                        }
                        
                        // Connect track to artist
                        let mut track_artist_query = query(
                            "MATCH (t:Track {spotify_id: $track_id})
                             MATCH (a:Artist {spotify_id: $artist_id})
                             MERGE (t)-[:PERFORMED_BY]->(a)"
                        );
                        track_artist_query = track_artist_query
                            .param("track_id", track_id.clone())
                            .param("artist_id", artist_id.clone());
                        
                        if let Err(e) = app_state.db.run(track_artist_query).await {
                            return Err(ServerFnError::ServerError(format!("Failed to connect track to artist: {}", e)));
                        }
                        
                        imported_artists += 1;
                    }
                }
            }
        }
    }
    
    let result = format!(
        "✅ Successfully imported playlist '{}'\n📊 {} tracks, {} artist relationships\n🕒 Import completed",
        playlist.name, imported_tracks, imported_artists
    );
    
    tracing::info!("Completed import: {}", result);
    Ok(result)
}

/// Check if a playlist is already imported in the database
#[server(CheckPlaylistImported)]
pub async fn check_playlist_imported(playlist_id: String) -> Result<bool, ServerFnError> {
    let user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let mut check_query = query(
        "MATCH (u:User {spotify_id: $user_id})-[:IMPORTED]->(p:Playlist {spotify_id: $playlist_id})
         RETURN p.spotify_id as id LIMIT 1"
    );
    check_query = check_query
        .param("user_id", user.spotify_id)
        .param("playlist_id", playlist_id);
    
    let mut result = match app_state.db.execute(check_query).await {
        Ok(result) => result,
        Err(e) => return Err(ServerFnError::ServerError(format!("Failed to check playlist: {}", e))),
    };
    
    Ok(result.next().await.is_ok())
}

/// Get import status for multiple playlists
#[server(GetPlaylistImportStatus)]
pub async fn get_playlist_import_status(playlist_ids: Vec<String>) -> Result<Vec<(String, bool)>, ServerFnError> {
    let user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let mut status_list = Vec::new();
    
    for playlist_id in playlist_ids {
        let mut check_query = query(
            "MATCH (u:User {spotify_id: $user_id})-[:IMPORTED]->(p:Playlist {spotify_id: $playlist_id})
             RETURN p.spotify_id as id LIMIT 1"
        );
        check_query = check_query
            .param("user_id", user.spotify_id.clone())
            .param("playlist_id", playlist_id.clone());
        
        let mut result = match app_state.db.execute(check_query).await {
            Ok(result) => result,
            Err(e) => return Err(ServerFnError::ServerError(format!("Failed to check playlist: {}", e))),
        };
        
        let is_imported = result.next().await.is_ok();
        status_list.push((playlist_id, is_imported));
    }
    
    Ok(status_list)
}
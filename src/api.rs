use std::{vec, collections::HashMap};
use dioxus::prelude::*;

use reqwest::Client;
use crate::{api_models::{AudioFeaturesResponse, NewPlaylistDetails, SpotifyAudioFeatures, SpotifyPlaylistItem, SpotifyPlaylistTrackResponse, SpotifyPlaylistsResponse, SpotifyTrackItem, SpotifyUserProfile}};

#[cfg(feature="server")]
use crate::server::AppState;

#[cfg(feature="server")]
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[cfg(feature="server")]
use rand::{thread_rng, seq::SliceRandom};

#[cfg(feature="server")]
use neo4rs::query;

#[cfg(feature="server")]
use crate::auth::helpers::{get_current_user, require_auth, spotify_api_call};



#[server(GetAccessToken)]
pub async fn get_access_token() -> Result<String, ServerFnError> {
    let user = require_auth().await?;
    Ok(user.access_token)
}

#[server(CheckAuth)]
pub async fn check_auth() -> Result<bool, ServerFnError> {
    Ok(get_current_user().await.is_some())
}
#[server(Logout)]
pub async fn logout() -> Result<(), ServerFnError> {
    if let Some(user) = get_current_user().await {
        let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
        
        // Delete all sessions for this user
        let mut query = query(
            "MATCH (u:User {spotify_id: $id})-[r:HAS_SESSION]->(s:Session)
             DETACH DELETE s"
        );
        query = query.param("id", user.spotify_id);

        if let Err(e) = app_state.db.run(query).await {
            tracing::error!("Failed to delete session from DB: {}", e);
        }
    }
    
    Ok(())
}

#[server(GetSpotifyUserData)]
pub async fn get_spotify_user_profile() -> Result<SpotifyUserProfile, ServerFnError>{
    spotify_api_call(|access_token| async move {
        let client = Client::new();
        let profile_endpoint = "https://api.spotify.com/v1/me";

        match client
            .get(profile_endpoint)
            .bearer_auth(access_token)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success(){
                    match response.json::<SpotifyUserProfile>().await {
                        Ok(profile) => Ok(profile),
                        Err(e) => {
                            tracing::error!("failed to parse user profile json {}", e);
                            Err((500, format!("failed to parse spotify profile: {}", e)))
                        }
                    }
                } else{
                    let status = response.status().as_u16();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown Error".to_string());
                    tracing::error!("failed to get user profile from spotify, status:{}, error:{}", status, error_text);
                    Err((status, error_text))
                }
            }
            Err(e) => {
                tracing::error!("Network Error while fetching user profile: {}",e);
                Err((500, format!("Network Error: {}", e)))
            }
        }
    }).await
}

#[server(GetSpotifyUserId)]
pub async fn get_spotify_user_id() -> Result<String, ServerFnError>{
    let profile = get_spotify_user_profile().await?;
    Ok(profile.id)
}

#[server(GetSpotifyUserPlaylistsPage)]
pub async fn get_spotify_user_playlists_page(limit: u32, offset:u32) -> Result<SpotifyPlaylistsResponse, ServerFnError>{
    tracing::info!("Attempting spotify user playlists page offset: {}", offset);

    spotify_api_call(|access_token| async move {
        let client = Client::new();
        let mut playlist_url = reqwest::Url::parse("https://api.spotify.com/v1/me/playlists").unwrap();
        playlist_url.query_pairs_mut()
            .append_pair("limit",&limit.to_string())
            .append_pair("offset", &offset.to_string());

        match client
            .get(playlist_url)
            .bearer_auth(&access_token)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success(){
                    match response.json::<SpotifyPlaylistsResponse>().await {
                        Ok(page_data) => {
                            tracing::info!(
                                "Successfully fetched page of {} playlists. Offset: {}",
                                page_data.items.len(),
                                page_data.offset);
                            Ok(page_data)
                        }
                        Err(e) => {
                            tracing::error!("Failed to parse playlist json: {}", e);
                            Err((500, format!("failed to parse spotify playlists: {}", e)))
                        }
                    }
                } else {
                    let status = response.status().as_u16();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown Error".to_string());
                    tracing::error!("failed to get user playlists from spotify, status:{}, error:{}", status, error_text);
                    Err((status, error_text))
                }
            }
            Err(e) => {
                tracing::error!("Network Error while fetching user playlists: {}",e);
                Err((500, format!("Network Error: {}", e)))
            }
        }
    }).await
}

#[server(GetSpotifyUserPlaylistsAll)]
pub async fn get_spotify_user_playlists_all() -> Result<Vec<SpotifyPlaylistItem>, ServerFnError>{
    
    let mut all_playlists:Vec<SpotifyPlaylistItem> = vec![];

    let mut current_limit: u32 = 50;
    let mut current_offset: u32 = 0;

    loop {
        match get_spotify_user_playlists_page(current_limit, current_offset).await {
            Ok(page_response) =>{
                // if page_response.items.is_empty() && page_response.next.is_none(){
                //     tracing::info!("No more playlists to fetch.");
                //     break;
                // }
                let num_items = page_response.items.len();
                all_playlists.extend(page_response.items);
                tracing::info!("Page: {}, Fetched {} playlists, total now: {}. Offset was: {}, total playlists to fetch:{}",
                    current_offset/50,
                    num_items,
                    all_playlists.len(),
                    current_offset,
                    page_response.total
                );

               let next_url = match page_response.next {
                    Some(s) => s,
                    None => break
                };
                let url = reqwest::Url::parse(&next_url)?;
                let params: HashMap<_,_> = url.query_pairs().into_owned().collect();


                current_offset = match params.get("offset") {
                    Some(offset_str) => match offset_str.parse::<u32>() {
                        Ok(num) => num,
                        Err(e) => {
                            tracing::error!("Failed to parse 'offset' from next_url query ('{}'): {}", offset_str, e);
                            return Err(ServerFnError::ServerError(format!("Invalid 'offset' in next URL: {}", e)));
                        }
                    },
                    None => {
                        tracing::warn!("'offset' not found in next_url query: {}. Assuming end or error.", next_url);
                        return Err(ServerFnError::ServerError("Missing 'offset' in Spotify's next URL".to_string()));
                    }
                };
                current_limit = match params.get("limit") {
                    Some(limit_str) => match limit_str.parse::<u32>() {
                        Ok(num) => num,
                        Err(e) => {
                            tracing::error!("Failed to parse 'offset' from next_url query ('{}'): {}", limit_str, e);
                            return Err(ServerFnError::ServerError(format!("Invalid 'offset' in next URL: {}", e)));
                        }
                    },
                    None => {
                        tracing::warn!("'offset' not found in next_url query: {}. Assuming end or error.", next_url);
                        return Err(ServerFnError::ServerError("Missing 'offset' in Spotify's next URL".to_string()));
                    }
                };
            
            }
            Err(e) =>{
                tracing::error!("error fetcing page of playlists: {}",e);
                return Err(e)
            }
        }
    }

    let mut unique_checker = std::collections::HashSet::new(); 
    all_playlists.retain(|p| unique_checker.insert(p.id.clone()));
    tracing::info!("Finished fetching. Total playlists retrieved: {}", all_playlists.len());
    Ok(all_playlists)
}

#[server(GetSpotifyPlaylistTracksAll)]
pub async fn get_spotify_playlist_tracks_all(playlist_id: String) -> Result<Vec<SpotifyTrackItem>,ServerFnError>{
    tracing::info!("Attempting to get tracks for playlist:{}",playlist_id);
    

    let mut all_tracks: Vec<SpotifyTrackItem> = vec![];
    let mut page_tracks: Vec<SpotifyTrackItem> = vec![];
    let mut current_offset: u32 = 0;
    let LIMIT :u32 = 50;
    
    loop {
        match get_spotify_playlist_tracks_page(playlist_id.clone(), LIMIT, current_offset).await {
            Ok(page_response) => {
                if page_response.items.is_empty() && page_response.next.is_none(){
                    tracing::info!("No more tracks on this page and no more next page");
                    break;
                }
                let num_items = page_response.items.len();
                page_tracks = page_response.items
                    .into_iter()
                    .filter_map(|item_wrapper| item_wrapper.track)
                    .collect();
                all_tracks.extend(page_tracks);

                tracing::info!("Page: {}, Fetched {} tracks, total now: {}. Offset was: {}, total playlists to fetch:{}",
                                    current_offset/LIMIT,
                                    num_items,
                                    all_tracks.len(),
                                    current_offset,
                                    page_response.total
                                );

                let next_url = match page_response.next {
                    Some(s) => s,
                    None => break
                };
                let url = reqwest::Url::parse(&next_url)?;
                let params: HashMap<_,_> = url.query_pairs().into_owned().collect();


                current_offset = match params.get("offset") {
                    Some(offset_str) => match offset_str.parse::<u32>() {
                        Ok(num) => num,
                        Err(e) => {
                            tracing::error!("Failed to parse 'offset' from next_url query ('{}'): {}", offset_str, e);
                            return Err(ServerFnError::ServerError(format!("Invalid 'offset' in next URL: {}", e)));
                        }
                    },
                    None => {
                        tracing::warn!("'offset' not found in next_url query: {}. Assuming end or error.", next_url);
                        return Err(ServerFnError::ServerError("Missing 'offset' in Spotify's next URL".to_string()));
                    }
                };

            }
            Err(e) => {
                tracing::error!("Error fetching page of tracks:{}",e);
                return Err(e)
            }
        }
    }
    tracing::info!("Finished fetching. Total tracks retrieved: {}", all_tracks.len());
    Ok(all_tracks)

}


#[server(GetSpotifyPlaylistTracksPage)]
pub async fn get_spotify_playlist_tracks_page(playlist_id: String, limit: u32, offset:u32) -> Result<SpotifyPlaylistTrackResponse, ServerFnError>{
    tracing::info!("Attempting spotify playlist tracks page offset: {}", offset);

    spotify_api_call(|access_token| {
        let playlist_id = playlist_id.clone();
        async move {
        const FIELDS: &str = "items(track(id,name,uri,artists(id,name))),limit,offset,total,next";

        let client = Client::new();
        let mut tracks_url = reqwest::Url::parse(
            format!("https://api.spotify.com/v1/playlists/{}/tracks",playlist_id).as_str()).unwrap();
        tracks_url.query_pairs_mut()
            .append_pair("offset", &offset.to_string())
            .append_pair("limit", &limit.to_string())
            .append_pair("fields", FIELDS);

        match client
            .get(tracks_url)
            .bearer_auth(access_token)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success(){
                    match response.json::<SpotifyPlaylistTrackResponse>().await {
                        Ok(page_data) => {
                            tracing::info!(
                                "Successfully fetched page of {} tracks. Offset: {}",
                                page_data.items.len(),
                                page_data.offset);
                            Ok(page_data)
                        }
                        Err(e) => {
                            tracing::error!("Failed to parse playlist json: {}", e);
                            Err((500, format!("failed to parse spotify playlists tracks: {}", e)))
                        }
                    }
                } else{
                    let status = response.status().as_u16();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown Error".to_string());
                    tracing::error!("failed to get playlist tracks from spotify, status:{}, error:{}", status, error_text);
                    Err((status, error_text))
                }
            }
            Err(e) => {
                tracing::error!("Network Error while fetching user playlists: {}",e);
                Err((500, format!("Network Error: {}", e)))
            }
        }
        }
    }).await
}

#[server(GetSpotifyPlaylist)]
pub async fn get_spotify_playlist(playlist_id: String) -> Result<SpotifyPlaylistItem, ServerFnError> {
    tracing::info!("Attempting to get playlist details for ID: {}", playlist_id);
    
    spotify_api_call(|access_token| {
        let playlist_id = playlist_id.clone();
        async move {
        let client = Client::new();
        let playlist_url = format!("https://api.spotify.com/v1/playlists/{}", playlist_id);
        
        match client
            .get(&playlist_url)
            .bearer_auth(access_token)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<SpotifyPlaylistItem>().await {
                        Ok(playlist) => {
                            tracing::info!("Successfully fetched playlist: {}", playlist.name);
                            Ok(playlist)
                        }
                        Err(e) => {
                            tracing::error!("Failed to parse playlist json: {}", e);
                            Err((500, format!("Failed to parse Spotify playlist: {}", e)))
                        }
                    }
                } else {
                    let status = response.status().as_u16();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown Error".to_string());
                    tracing::error!("Failed to get playlist from Spotify, status:{}, error:{}", status, error_text);
                    Err((status, error_text))
                }
            }
            Err(e) => {
                tracing::error!("Network Error while fetching playlist: {}", e);
                Err((500, format!("Network Error: {}", e)))
            }
        }
        }
    }).await
}

#[server(ShuffleAndSavePlaylist)] // Reverting to this name
pub async fn shuffle_and_save_new_playlist(
    original_playlist_id: String,
    original_playlist_name: String,
) -> Result<NewPlaylistDetails, ServerFnError> { // Use default ServerFnError for now
    #[cfg(feature = "server")]
    {
        tracing::info!("API: Server-side shuffle for playlist: '{}' (ID: {})", original_playlist_name, original_playlist_id);

        // 1. Get Current User's Spotify ID
        let user_id = match get_spotify_user_id().await {
            Ok(id) => id,
            Err(e) => return Err(ServerFnError::ServerError(format!("Failed to get user ID: {}", e))),
        };
        tracing::info!("API: Target user ID: {}", user_id);

        // 2. Fetch All Tracks for the Original Playlist
        let tracks_for_shuffling = match get_spotify_playlist_tracks_all(original_playlist_id.clone()).await {
            Ok(tracks) => tracks,
            Err(e) => return Err(ServerFnError::ServerError(format!("Failed to fetch tracks for '{}': {}", original_playlist_name, e))),
        };

        if tracks_for_shuffling.is_empty() {
            return Err(ServerFnError::ServerError(format!("Playlist '{}' is empty.", original_playlist_name)));
        }
        tracing::info!("API: Fetched {} tracks for '{}'.", tracks_for_shuffling.len(), original_playlist_name);

        // 3. Extract Track URIs AND THEN Shuffle them
        let mut track_uris: Vec<String> = tracks_for_shuffling
            .into_iter()
            .filter_map(|t| t.id.map(|id_val| format!("spotify:track:{}", id_val)))
            .collect();

        if track_uris.is_empty() {
            return Err(ServerFnError::ServerError("No valid track URIs found in the playlist.".to_string()));
        }

        // --- Perform shuffle synchronously here ---
        { // Create a limited scope for rng if using thread_rng
            let mut rng = thread_rng(); // Create RNG here
            track_uris.shuffle(&mut rng); // Shuffle the mutable vector
        } // rng goes out of scope; if it was thread_rng, its non-Send parts are dropped.
          // If thread_rng() still gives Send issues due to the outer async fn,
          // use `let mut rng = rand::rngs::StdRng::from_entropy();`
        tracing::info!("API: Shuffled {} track URIs.", track_uris.len());
        // --- End shuffle ---


        // 4. Create a New Playlist
        let new_playlist_name = format!("{} - TRUE SHUFFLED", original_playlist_name);
        let created_playlist_data: SpotifyPlaylistItem = spotify_api_call(|access_token| {
            let user_id = user_id.clone();
            let new_playlist_name = new_playlist_name.clone();
            let original_playlist_name = original_playlist_name.clone();
            async move {
                #[derive(serde::Serialize)]
                struct CreatePlaylistPayload<'a> { name: &'a str, public: bool, description: String }
                let create_payload = CreatePlaylistPayload {
                    name: &new_playlist_name,
                    public: false,
                    description: format!(
                        "A true random shuffle of '{}'!",
                        original_playlist_name
                    ),
                };
                let create_playlist_url = format!("https://api.spotify.com/v1/users/{}/playlists", user_id);
                tracing::info!("API: Creating new playlist: {}", new_playlist_name);

                let client = Client::new();
                match client
                    .post(&create_playlist_url)
                    .bearer_auth(access_token)
                    .json(&create_payload)
                    .send().await {
                        Ok(response) => {
                            if response.status().is_success() || response.status().as_u16() == 201 {
                                match response.json::<SpotifyPlaylistItem>().await {
                                    Ok(data) => Ok(data),
                                    Err(e) => Err((500, format!("API: Parse new playlist response error: {}", e))),
                                }
                            } else {
                                let status = response.status().as_u16();
                                let error_text = response.text().await.unwrap_or_default();
                                Err((status, format!("API: Spotify error creating playlist: {}", error_text)))
                            }
                        }
                        Err(e) => Err((500, format!("API: Network error creating playlist: {}", e))),
                    }
            }
        }).await?;
        let new_playlist_id = created_playlist_data.id.clone();
        tracing::info!("API: New playlist created '{}' (ID: {})", new_playlist_name, new_playlist_id);

        // 5. Add Shuffled Tracks to the New Playlist (in batches)
        if !track_uris.is_empty() {
            for chunk_of_uris in track_uris.chunks(100) {
                let chunk_vec: Vec<String> = chunk_of_uris.to_vec();
                spotify_api_call(|access_token| {
                    let new_playlist_id = new_playlist_id.clone();
                    let chunk_vec = chunk_vec.clone();
                    async move {
                        #[derive(serde::Serialize)]
                        struct AddTracksPayload {
                            uris: Vec<String>,
                        }
                        let add_payload = AddTracksPayload {
                            uris: chunk_vec.clone(),
                        };
                        let add_tracks_url = format!("https://api.spotify.com/v1/playlists/{}/tracks", new_playlist_id);
                        tracing::info!(
                            "API: Adding {} tracks to new playlist ID {}",
                            chunk_vec.len(),
                            new_playlist_id
                        );
                        
                        let client = Client::new();
                        match client.post(&add_tracks_url).bearer_auth(access_token).json(&add_payload).send().await {
                            Ok(response) => {
                                if response.status().is_success() {
                                    Ok(())
                                } else {
                                    let status = response.status().as_u16();
                                    let error_text = response.text().await.unwrap_or_default();
                                    Err((status, format!("API: Error adding tracks: {}", error_text)))
                                }
                            }
                            Err(e) => Err((500, format!("API: Network error adding tracks: {}", e))),
                        }
                    }
                }).await?;
                
                if track_uris.len() > 100 && chunk_of_uris.len() == 100 { // Avoid sleep if only one chunk or last small chunk
                    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                }
            }
        }
        tracing::info!("API: All tracks added to new playlist: {}", new_playlist_name);

        // 6. Copy the original playlist's image to the new playlist
        // First, get the original playlist details to access its images
        match get_spotify_playlist(original_playlist_id.clone()).await {
            Ok(original_playlist) => {
                if let Some(images) = &original_playlist.images {
                    if !images.is_empty() {
                        // Typically we want the first image (usually the largest)
                        let original_image_url = &images[0].url;
                        tracing::info!("API: Copying image from original playlist: {}", original_image_url);
                        
                        // Fetch the image from the URL
                        let client = Client::new();
                        match client.get(original_image_url).send().await {
                            Ok(img_response) => {
                                if img_response.status().is_success() {
                                    // Get the image bytes
                                    match img_response.bytes().await {
                                        Ok(img_bytes) => {
                                            // Convert to base64 (required by Spotify API)
                                            let base64_img = STANDARD.encode(img_bytes);
                                            
                                            // Call Spotify API to update playlist image
                                            let upload_result: Result<(), ServerFnError> = spotify_api_call(|access_token| {
                                                let new_playlist_id = new_playlist_id.clone();
                                                let base64_img = base64_img.clone();
                                                async move {
                                                    let upload_image_url = format!("https://api.spotify.com/v1/playlists/{}/images", new_playlist_id);
                                                    let client = Client::new();
                                                    match client
                                                        .put(&upload_image_url)
                                                        .bearer_auth(access_token)
                                                        .header("Content-Type", "image/jpeg")
                                                        .body(base64_img)
                                                        .send()
                                                        .await
                                                    {
                                                        Ok(upload_response) => {
                                                            if upload_response.status().is_success() {
                                                                tracing::info!("API: Successfully copied image to new playlist");
                                                                Ok(())
                                                            } else {
                                                                let status = upload_response.status().as_u16();
                                                                let err_text = upload_response.text().await.unwrap_or_default();
                                                                Err((status, format!("API: Failed to upload image: {}", err_text)))
                                                            }
                                                        },
                                                        Err(e) => {
                                                            Err((500, format!("API: Network error uploading image: {}", e)))
                                                        }
                                                    }
                                                }
                                            }).await;
                                            
                                            match upload_result {
                                                Ok(_) => {},
                                                Err(e) => {
                                                    tracing::error!("API: Failed to upload image: {}", e);
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            tracing::error!("API: Failed to get image bytes: {}", e);
                                        }
                                    }
                                } else {
                                    tracing::error!("API: Failed to fetch image, status: {}", img_response.status());
                                }
                            },
                            Err(e) => {
                                tracing::error!("API: Network error fetching image: {}", e);
                            }
                        }
                    } else {
                        tracing::info!("API: Original playlist has no images");
                    }
                } else {
                    tracing::info!("API: Original playlist has no images array");
                }
            },
            Err(e) => {
                tracing::error!("API: Failed to get original playlist details: {}", e);
            }
        }
        
        // 7. Return Success
        let web_url = format!("https://open.spotify.com/playlist/{}", new_playlist_id);
        Ok(NewPlaylistDetails {
            id: new_playlist_id,
            name: new_playlist_name,
            external_url: web_url,
        })
    }
}

/// Fetch audio features for multiple tracks (up to 100 at once)
/// NOTE: Due to Spotify API deprecation (Nov 27, 2024), this now generates mock data for demo purposes
#[server(GetAudioFeatures)]
pub async fn get_audio_features(track_ids: Vec<String>) -> Result<Vec<Option<SpotifyAudioFeatures>>, ServerFnError> {
    let _user = require_auth().await?;
    
    // Filter out empty IDs and limit to 100 tracks per Spotify API limits
    let valid_ids: Vec<&String> = track_ids.iter()
        .filter(|id| !id.is_empty())
        .take(100)
        .collect();
    
    if valid_ids.is_empty() {
        return Ok(vec![]);
    }
    
    tracing::info!("Generating mock audio features for {} tracks (Spotify API deprecated)", valid_ids.len());
    
    // Generate realistic mock audio features for demo purposes
    let mock_features: Vec<Option<SpotifyAudioFeatures>> = valid_ids.iter().map(|track_id| {
        Some(generate_mock_audio_features(track_id))
    }).collect();
    
    tracing::info!("Generated {} mock audio features", mock_features.len());
    Ok(mock_features)
}

/// Generate realistic mock audio features based on track ID hash
fn generate_mock_audio_features(track_id: &str) -> SpotifyAudioFeatures {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    // Use track ID as seed for consistent but pseudo-random values
    let mut hasher = DefaultHasher::new();
    track_id.hash(&mut hasher);
    let seed = hasher.finish();
    
    // Generate values that look realistic but consistent for the same track
    let normalized_seed = (seed % 10000) as f64 / 10000.0;
    
    // Create realistic audio features based on seeded randomness
    SpotifyAudioFeatures {
        id: track_id.to_string(),
        acousticness: (normalized_seed * 0.8 + 0.1), // 0.1-0.9
        danceability: ((seed % 7919) as f64 / 7919.0 * 0.7 + 0.2), // 0.2-0.9
        duration_ms: ((seed % 240000) + 60000) as u32, // 1-5 minutes
        energy: ((seed % 8171) as f64 / 8171.0 * 0.8 + 0.1), // 0.1-0.9
        instrumentalness: ((seed % 9973) as f64 / 9973.0 * 0.6), // 0.0-0.6
        key: ((seed % 12) as i32), // 0-11 (musical keys)
        liveness: ((seed % 7001) as f64 / 7001.0 * 0.4 + 0.05), // 0.05-0.45
        loudness: -((seed % 40) as f64 + 5.0), // -45 to -5 dB
        mode: ((seed % 2) as i32), // 0 (minor) or 1 (major)
        speechiness: ((seed % 6007) as f64 / 6007.0 * 0.3), // 0.0-0.3
        tempo: ((seed % 140) + 60) as f64, // 60-200 BPM
        time_signature: [3, 4, 4, 4, 5][(seed % 5) as usize], // Mostly 4/4
        valence: ((seed % 8537) as f64 / 8537.0 * 0.8 + 0.1), // 0.1-0.9
        track_type: "audio_features".to_string(),
        uri: format!("spotify:track:{}", track_id),
        track_href: format!("https://api.spotify.com/v1/tracks/{}", track_id),
        analysis_url: format!("https://api.spotify.com/v1/audio-analysis/{}", track_id),
    }
}

/// Fetch audio features for a single track
#[server(GetSingleAudioFeatures)]
pub async fn get_single_audio_features(track_id: String) -> Result<Option<SpotifyAudioFeatures>, ServerFnError> {
    let features = get_audio_features(vec![track_id]).await?;
    Ok(features.into_iter().next().flatten())
}


/// Test function: Fetch playlist tracks and their audio features
#[server(TestPlaylistAudioFeatures)]
pub async fn test_playlist_audio_features(playlist_id: String) -> Result<String, ServerFnError> {
    tracing::info!("Testing audio features for playlist: {}", playlist_id);
    
    // 1. Get playlist details
    let playlist = get_spotify_playlist(playlist_id.clone()).await?;
    tracing::info!("Playlist: {} - {}", playlist.name, playlist.id);
    
    // 2. Get all tracks from the playlist
    let tracks = get_spotify_playlist_tracks_all(playlist_id).await?;
    tracing::info!("Found {} tracks in playlist", tracks.len());
    
    if tracks.is_empty() {
        return Ok(format!("Playlist '{}' is empty", playlist.name));
    }
    
    // 3. Extract track IDs for audio features (limit to first 10 for testing)
    let track_ids: Vec<String> = tracks.iter()
        .filter_map(|track| track.id.clone())
        .take(10)
        .collect();
    
    if track_ids.is_empty() {
        return Ok(format!("No valid track IDs found in playlist '{}'", playlist.name));
    }
    
    tracing::info!("Getting audio features for {} tracks", track_ids.len());
    
    // 4. Fetch audio features
    let audio_features = get_audio_features(track_ids.clone()).await?;
    
    // 5. Create summary report
    let mut report = format!("🎵 Audio Features Test for '{}'\n", playlist.name);
    report.push_str("⚠️  Note: Using mock data (Spotify API deprecated audio features Nov 27, 2024)\n");
    report.push_str(&format!("📊 Analyzed {} out of {} tracks\n\n", audio_features.len(), tracks.len()));
    
    for (i, (track, features_opt)) in tracks.iter().zip(audio_features.iter()).take(10).enumerate() {
        report.push_str(&format!("{}. {}\n", i + 1, track.display_name()));
        
        if let Some(features) = features_opt {
            report.push_str(&format!("   {}\n", features.summary()));
            report.push_str(&format!("   Key: {}, Mode: {}, Time Signature: {}/4\n", 
                features.key, features.mode, features.time_signature));
        } else {
            report.push_str("   ❌ No audio features available\n");
        }
        report.push_str("\n");
    }
    
    // 6. Add statistics
    let valid_features: Vec<_> = audio_features.iter().filter_map(|f| f.as_ref()).collect();
    if !valid_features.is_empty() {
        let avg_tempo = valid_features.iter().map(|f| f.tempo).sum::<f64>() / valid_features.len() as f64;
        let avg_energy = valid_features.iter().map(|f| f.energy).sum::<f64>() / valid_features.len() as f64;
        let avg_valence = valid_features.iter().map(|f| f.valence).sum::<f64>() / valid_features.len() as f64;
        
        report.push_str("📈 Playlist Statistics:\n");
        report.push_str(&format!("   Average Tempo: {:.1} BPM\n", avg_tempo));
        report.push_str(&format!("   Average Energy: {:.1}/10\n", avg_energy * 10.0));
        report.push_str(&format!("   Average Valence: {:.1}/10\n", avg_valence * 10.0));
        report.push_str(&format!("   Features retrieved: {}/{}\n", valid_features.len(), track_ids.len()));
    }
    
    tracing::info!("Audio features test completed successfully");
    Ok(report)
}
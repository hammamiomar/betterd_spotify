use std::{vec, collections::HashMap};
use dioxus::prelude::*;

use reqwest::Client;
use crate::{api_models::{AudioFeaturesResponse, NewPlaylistDetails, LLMSongFeatures, SpotifyAudioFeatures, SpotifyPlaylistItem, SpotifyPlaylistTrackResponse, SpotifyPlaylistsResponse, SpotifyTrackItem, SpotifyUserProfile, NodeTypeInfo, NodeValue, PlaylistGenerationRequest, GeneratedPlaylistResult}};

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
            .map(|t| t.uri)
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

/// Call OpenRouter LLM to analyze song features
#[cfg(feature="server")]
async fn get_llm_song_features(tracks: Vec<SpotifyTrackItem>) -> Result<Vec<LLMSongFeatures>, anyhow::Error> {
    use std::env;
    use anyhow::anyhow;
    
    let api_key = env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow!("OPENROUTER_API_KEY environment variable not set"))?;
    
    if tracks.is_empty() {
        return Ok(vec![]);
    }
    
    tracing::info!("🔑 OpenRouter API key found, proceeding with LLM analysis");
    tracing::info!("📝 Processing {} tracks for semantic analysis", tracks.len());
    
    // Read the system prompt
    let system_prompt = include_str!("llm_system_prompt.txt");
    tracing::info!("📋 System prompt loaded ({} characters)", system_prompt.len());
    
    // Format tracks for LLM input - just artist and track name
    let track_data: Vec<serde_json::Value> = tracks.iter().map(|track| {
        serde_json::json!({
            "song_id": track.id,
            "artist": track.primary_artist(),
            "track_name": track.name
        })
    }).collect();
    
    let user_message = format!(
        "Analyze these songs and return the structured JSON array:\n{}",
        serde_json::to_string_pretty(&track_data).map_err(|e| anyhow!("Failed to serialize track data: {}", e))?
    );
    
    let request_body = serde_json::json!({
        "model": "moonshotai/kimi-k2",
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user", 
                "content": user_message
            }
        ],
        "max_tokens": 50000,
        "temperature": 0.1
    });
    
    tracing::info!("🚀 Sending request to OpenRouter API...");
    tracing::info!("📊 Request payload size: {} bytes", serde_json::to_string(&request_body).unwrap_or_default().len());
    
    let client = Client::new();
    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("HTTP-Referer", "https://betterd-spotify.com")
        .header("X-Title", "Betterd Spotify")
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send request to OpenRouter API: {}", e))?;
    
    tracing::info!("📡 Received response from OpenRouter (status: {})", response.status());
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "OpenRouter API error ({}): {}", status, error_text
        ));
    }
    
    let response_json: serde_json::Value = response.json().await
        .map_err(|e| anyhow!("Failed to parse OpenRouter response as JSON: {}", e))?;
    
    // Extract the content from OpenRouter response
    let content = response_json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .ok_or_else(|| anyhow!("Invalid OpenRouter response format - missing content field"))?;
    
    tracing::info!("LLM Response received: {} characters", content.len());
    tracing::info!("LLM Response content: {}", content);
    
    // Extract JSON from markdown code blocks if present
    let json_content = if content.trim().starts_with("```json") {
        tracing::info!("Detected markdown code block, extracting JSON");
        content
            .trim()
            .strip_prefix("```json")
            .and_then(|s| s.strip_suffix("```"))
            .unwrap_or(content)
            .trim()
    } else if content.trim().starts_with("```") {
        tracing::info!("Detected generic code block, extracting content");
        content
            .trim()
            .strip_prefix("```")
            .and_then(|s| s.strip_suffix("```"))
            .unwrap_or(content)
            .trim()
    } else {
        content.trim()
    };
    
    tracing::info!("Extracted JSON content: {}", json_content);
    
    // Parse the LLM response as JSON array
    let llm_responses: Vec<serde_json::Value> = serde_json::from_str(json_content)
        .map_err(|e| {
            tracing::error!("Failed to parse LLM response as JSON. Error: {}", e);
            tracing::error!("Extracted JSON content: {}", json_content);
            anyhow!("Failed to parse LLM response as JSON array: {}. Extracted content: {}", e, json_content)
        })?;
    
    // Convert to LLMSongFeatures structs
    let mut features = Vec::new();
    for (i, response) in llm_responses.iter().enumerate() {
        if i < tracks.len() {
            match parse_llm_response_to_features(response, &tracks[i]) {
                Ok(feature) => features.push(feature),
                Err(e) => {
                    tracing::error!("Failed to parse LLM response for track {}: {}", tracks[i].id, e);
                    features.push(create_fallback_features(&tracks[i]));
                }
            }
        }
    }
    
    tracing::info!("Successfully processed {} LLM song features", features.len());
    Ok(features)
}

/// Parse individual LLM response into LLMSongFeatures
#[cfg(feature="server")]
fn parse_llm_response_to_features(response: &serde_json::Value, track: &SpotifyTrackItem) -> Result<LLMSongFeatures, String> {
    use crate::api_models::{MetadataConfidence, GenreFeature, CulturalFeature, UserTag};
    
    // Parse metadata confidence
    let metadata_confidence = MetadataConfidence {
        level: response.get("metadata_confidence")
            .and_then(|c| c.get("level"))
            .and_then(|l| l.as_str())
            .unwrap_or("Medium")
            .to_string(),
        explanation: response.get("metadata_confidence")
            .and_then(|c| c.get("explanation"))
            .and_then(|e| e.as_str())
            .unwrap_or("No explanation provided")
            .to_string(),
    };
    
    // Parse genres
    let mut genres = Vec::new();
    if let Some(genre_tags) = response.get("identity").and_then(|i| i.get("genre_tags")).and_then(|g| g.as_array()) {
        let primary_genre = response.get("identity")
            .and_then(|i| i.get("primary_genre"))
            .and_then(|p| p.as_str());
        
        for tag in genre_tags {
            if let (Some(name), Some(confidence)) = (
                tag.get("tag").and_then(|t| t.as_str()),
                tag.get("confidence").and_then(|c| c.as_f64())
            ) {
                let is_primary = Some(name) == primary_genre;
                genres.push(GenreFeature {
                    name: name.to_string(),
                    confidence,
                    is_primary,
                });
            }
        }
    }
    
    // Parse cultural origins
    let mut cultural_origins = Vec::new();
    if let Some(cultural_tags) = response.get("identity").and_then(|i| i.get("cultural_origin_tags")).and_then(|c| c.as_array()) {
        for tag in cultural_tags {
            if let (Some(name), Some(confidence)) = (
                tag.get("tag").and_then(|t| t.as_str()),
                tag.get("confidence").and_then(|c| c.as_f64())
            ) {
                cultural_origins.push(CulturalFeature {
                    name: name.to_string(),
                    confidence,
                });
            }
        }
    }
    
    // Parse user facing tags
    let mut user_facing_tags = Vec::new();
    if let Some(tags) = response.get("affective_qualities").and_then(|a| a.get("user_facing_tags")).and_then(|u| u.as_array()) {
        for tag in tags {
            if let (Some(name), Some(confidence)) = (
                tag.get("tag").and_then(|t| t.as_str()),
                tag.get("confidence").and_then(|c| c.as_f64())
            ) {
                user_facing_tags.push(UserTag {
                    tag: name.to_string(),
                    confidence,
                });
            }
        }
    }
    
    // Extract instruments
    let instruments = response.get("instrumentation_and_timbre")
        .and_then(|i| i.get("primary_instruments"))
        .and_then(|p| p.as_array())
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect())
        .unwrap_or_default();
    
    Ok(LLMSongFeatures {
        song_id: track.id.clone(),
        metadata_confidence,
        genres,
        cultural_origins,
        primary_language: response.get("identity")
            .and_then(|i| i.get("primary_language"))
            .and_then(|l| l.as_str())
            .map(|s| s.to_string()),
        instruments,
        energy_level: response.get("affective_qualities")
            .and_then(|a| a.get("energy_level"))
            .and_then(|e| e.as_str())
            .map(|s| s.to_string()),
        valence: response.get("affective_qualities")
            .and_then(|a| a.get("valence"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        production_style: response.get("production_and_era")
            .and_then(|p| p.get("production_style"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string()),
        vocal_presence: response.get("vocal_characteristics")
            .and_then(|v| v.get("vocal_presence"))
            .and_then(|p| p.as_str())
            .map(|s| s.to_string()),
        user_facing_tags,
        rhythmic_feel: response.get("sonic_fingerprint")
            .and_then(|s| s.get("rhythmic_feel"))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string()),
        density: response.get("sonic_fingerprint")
            .and_then(|s| s.get("density"))
            .and_then(|d| d.as_str())
            .unwrap_or("Medium")
            .to_string(),
    })
}

/// Store LLM song features in Neo4j as graph nodes and relationships
#[cfg(feature="server")]
async fn store_song_features_in_graph(song_id: &str, features: &LLMSongFeatures) -> Result<(), anyhow::Error> {
    use anyhow::anyhow;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    tracing::info!("Storing song features for {} in Neo4j graph", song_id);
    
    // 1. Store/Update the Song node with basic properties
    let mut song_query = query(
        "MERGE (s:Song {id: $song_id})
         SET s.rhythmic_feel = $rhythmic_feel,
             s.density = $density,
             s.confidence_level = $confidence_level,
             s.confidence_explanation = $confidence_explanation"
    );
    song_query = song_query
        .param("song_id", song_id)
        .param("rhythmic_feel", features.rhythmic_feel.as_deref())
        .param("density", features.density.as_str())
        .param("confidence_level", features.metadata_confidence.level.as_str())
        .param("confidence_explanation", features.metadata_confidence.explanation.as_str());
    
    app_state.db.run(song_query).await
        .map_err(|e| anyhow::anyhow!("Failed to store song node in Neo4j: {}", e))?;
    
    // 2. Store genres and relationships
    for genre in &features.genres {
        let mut genre_query = query(
            "MERGE (g:Genre {name: $genre_name})
             WITH g
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[r:HAS_GENRE]->(g)
             SET r.confidence = $confidence,
                 r.is_primary = $is_primary"
        );
        genre_query = genre_query
            .param("song_id", song_id)
            .param("genre_name", genre.name.as_str())
            .param("confidence", genre.confidence)
            .param("is_primary", genre.is_primary);
        
        app_state.db.run(genre_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store genre relationship: {}", e))?;
    }
    
    // 3. Store cultural origins
    for culture in &features.cultural_origins {
        let mut culture_query = query(
            "MERGE (c:Culture {name: $culture_name})
             WITH c
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[r:ORIGINATED_FROM]->(c)
             SET r.confidence = $confidence"
        );
        culture_query = culture_query
            .param("song_id", song_id)
            .param("culture_name", culture.name.as_str())
            .param("confidence", culture.confidence);
        
        app_state.db.run(culture_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store culture relationship: {}", e))?;
    }
    
    // 4. Store primary language
    if let Some(language) = &features.primary_language {
        let mut lang_query = query(
            "MERGE (l:Language {name: $language})
             WITH l
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[:SUNG_IN]->(l)"
        );
        lang_query = lang_query
            .param("song_id", song_id)
            .param("language", language.as_str());
        
        app_state.db.run(lang_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store language relationship: {}", e))?;
    }
    
    // 5. Store instruments
    for instrument in &features.instruments {
        let mut instrument_query = query(
            "MERGE (i:Instrument {name: $instrument_name})
             WITH i
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[:FEATURES_INSTRUMENT]->(i)"
        );
        instrument_query = instrument_query
            .param("song_id", song_id)
            .param("instrument_name", instrument.as_str());
        
        app_state.db.run(instrument_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store instrument relationship: {}", e))?;
    }
    
    // 6. Store energy level
    if let Some(energy) = &features.energy_level {
        let mut energy_query = query(
            "MERGE (e:EnergyLevel {level: $energy_level})
             WITH e
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[:HAS_ENERGY_LEVEL]->(e)"
        );
        energy_query = energy_query
            .param("song_id", song_id)
            .param("energy_level", energy.as_str());
        
        app_state.db.run(energy_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store energy level relationship: {}", e))?;
    }
    
    // 7. Store valence
    if let Some(valence) = &features.valence {
        let mut valence_query = query(
            "MERGE (v:Valence {level: $valence_level})
             WITH v
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[:HAS_VALENCE]->(v)"
        );
        valence_query = valence_query
            .param("song_id", song_id)
            .param("valence_level", valence.as_str());
        
        app_state.db.run(valence_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store valence relationship: {}", e))?;
    }
    
    // 8. Store production style
    if let Some(production) = &features.production_style {
        let mut prod_query = query(
            "MERGE (p:ProductionStyle {name: $production_name})
             WITH p
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[:HAS_PRODUCTION_STYLE]->(p)"
        );
        prod_query = prod_query
            .param("song_id", song_id)
            .param("production_name", production.as_str());
        
        app_state.db.run(prod_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store production style relationship: {}", e))?;
    }
    
    // 9. Store vocal presence
    if let Some(vocal) = &features.vocal_presence {
        let mut vocal_query = query(
            "MERGE (v:VocalStyle {name: $vocal_name})
             WITH v
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[:HAS_VOCAL_STYLE]->(v)"
        );
        vocal_query = vocal_query
            .param("song_id", song_id)
            .param("vocal_name", vocal.as_str());
        
        app_state.db.run(vocal_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store vocal style relationship: {}", e))?;
    }
    
    // 10. Store user facing tags
    for tag in &features.user_facing_tags {
        let mut tag_query = query(
            "MERGE (t:UserTag {name: $tag_name})
             WITH t
             MATCH (s:Song {id: $song_id})
             MERGE (s)-[r:HAS_USER_TAG]->(t)
             SET r.confidence = $confidence"
        );
        tag_query = tag_query
            .param("song_id", song_id)
            .param("tag_name", tag.tag.as_str())
            .param("confidence", tag.confidence);
        
        app_state.db.run(tag_query).await
            .map_err(|e| anyhow::anyhow!("Failed to store user tag relationship: {}", e))?;
    }
    
    tracing::info!("Successfully stored all song features for {} in Neo4j", song_id);
    Ok(())
}

/// Create fallback features when LLM parsing fails
#[cfg(feature="server")]
fn create_fallback_features(track: &SpotifyTrackItem) -> LLMSongFeatures {
    use crate::api_models::{MetadataConfidence, GenreFeature};
    
    LLMSongFeatures {
        song_id: track.id.clone(),
        metadata_confidence: MetadataConfidence {
            level: "Very Low".to_string(),
            explanation: "LLM parsing failed, using fallback data".to_string(),
        },
        genres: vec![GenreFeature {
            name: "Unknown".to_string(),
            confidence: 0.0,
            is_primary: true,
        }],
        cultural_origins: vec![],
        primary_language: None,
        instruments: vec![],
        energy_level: None,
        valence: None,
        production_style: None,
        vocal_presence: None,
        user_facing_tags: vec![],
        rhythmic_feel: None,
        density: "Medium".to_string(),
    }
}

/// Fetch audio features for multiple tracks (up to 100 at once)
/// Now uses LLM-powered analysis instead of mock data
#[server(GetAudioFeatures)]
pub async fn get_audio_features(track_ids: Vec<String>) -> Result<Vec<Option<SpotifyAudioFeatures>>, ServerFnError> {
    let _user = require_auth().await?;
    
    // Filter out empty IDs and limit to reasonable batch size for LLM
    let valid_ids: Vec<String> = track_ids.into_iter()
        .filter(|id| !id.is_empty())
        .take(10) // Reduce batch size for LLM calls
        .collect();
    
    if valid_ids.is_empty() {
        return Ok(vec![]);
    }
    
    tracing::info!("Getting LLM audio features for {} tracks", valid_ids.len());
    
    // We need to get track details first (artist names, etc.) for the LLM
    // For now, create minimal track objects from IDs
    // TODO: In a full implementation, we'd fetch track details from Spotify first
    let tracks: Vec<SpotifyTrackItem> = valid_ids.iter().map(|id| {
        SpotifyTrackItem {
            id: id.clone(),
            uri: format!("spotify:track:{}", id),
            name: "Unknown Track".to_string(), // Placeholder - should fetch from Spotify
            artists: vec![], // Placeholder - should fetch from Spotify
            album: None,
            duration_ms: None,
            explicit: None,
            audio_features: None,
        }
    }).collect();
    
    // For now, return empty results since we need actual track metadata for LLM
    // This will be properly implemented when we have track fetching
    tracing::warn!("LLM audio features not fully implemented yet - track metadata needed");
    let empty_results: Vec<Option<SpotifyAudioFeatures>> = (0..valid_ids.len())
        .map(|_| None)
        .collect();
    
    Ok(empty_results)
}


/// Fetch audio features for a single track
#[server(GetSingleAudioFeatures)]
pub async fn get_single_audio_features(track_id: String) -> Result<Option<SpotifyAudioFeatures>, ServerFnError> {
    let features = get_audio_features(vec![track_id]).await?;
    Ok(features.into_iter().next().flatten())
}


/// Find songs by language using Neo4j graph query
#[server(FindSongsByLanguage)]
pub async fn find_songs_by_language(language: String) -> Result<Vec<String>, ServerFnError> {
    let _user = require_auth().await?; 
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let mut query = query(
        "MATCH (s:Song)-[:SUNG_IN]->(l:Language {name: $language})
         RETURN s.id as song_id
         LIMIT 50"
    );
    query = query.param("language", language.as_str());
    
    let mut result = app_state.db.execute(query).await
        .map_err(|e| ServerFnError::new(format!("Failed to query songs by language: {}", e)))?;
    
    let mut song_ids = Vec::new();
    while let Ok(Some(row)) = result.next().await {
        if let Ok(song_id) = row.get::<String>("song_id") {
            song_ids.push(song_id);
        }
    }
    
    tracing::info!("Found {} songs in language: {}", song_ids.len(), language);
    Ok(song_ids)
}

/// Find songs by genre using Neo4j graph query
#[server(FindSongsByGenre)]
pub async fn find_songs_by_genre(genre: String, min_confidence: f64) -> Result<Vec<String>, ServerFnError> {
    let _user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let mut query = query(
        "MATCH (s:Song)-[r:HAS_GENRE]->(g:Genre {name: $genre})
         WHERE r.confidence >= $min_confidence
         RETURN s.id as song_id, r.confidence as confidence
         ORDER BY r.confidence DESC
         LIMIT 50"
    );
    query = query.param("genre", genre.as_str()).param("min_confidence", min_confidence);
    
    let mut result = app_state.db.execute(query).await
        .map_err(|e| ServerFnError::new(format!("Failed to query songs by genre: {}", e)))?;
    
    let mut song_ids = Vec::new();
    while let Ok(Some(row)) = result.next().await {
        if let Ok(song_id) = row.get::<String>("song_id") {
            song_ids.push(song_id);
        }
    }
    
    tracing::info!("Found {} songs in genre {} with confidence >= {}", song_ids.len(), genre, min_confidence);
    Ok(song_ids)
}

/// Find songs by energy level and culture combination
#[server(FindSongsByEnergyAndCulture)]
pub async fn find_songs_by_energy_and_culture(energy: String, culture: String) -> Result<Vec<String>, ServerFnError> {
    let _user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let mut query = query(
        "MATCH (s:Song)-[:HAS_ENERGY_LEVEL]->(e:EnergyLevel {level: $energy})
         MATCH (s)-[:ORIGINATED_FROM]->(c:Culture {name: $culture})
         RETURN s.id as song_id
         LIMIT 50"
    );
    query = query.param("energy", energy.as_str()).param("culture", culture.as_str());
    
    let mut result = app_state.db.execute(query).await
        .map_err(|e| ServerFnError::new(format!("Neo4j query failed: {}", e)))?;
    
    let mut song_ids = Vec::new();
    while let Ok(Some(row)) = result.next().await {
        if let Ok(song_id) = row.get::<String>("song_id") {
            song_ids.push(song_id);
        }
    }
    
    tracing::info!("Found {} songs with energy {} from culture {}", song_ids.len(), energy, culture);
    Ok(song_ids)
}

/// Find similar songs to a seed song based on shared features
#[server(FindSimilarSongs)]
pub async fn find_similar_songs(seed_song_id: String, min_shared_features: i64) -> Result<Vec<String>, ServerFnError> {
    let _user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let mut query = query(
        "MATCH (seed:Song {id: $seed_song_id})
         MATCH (seed)-[:HAS_GENRE|ORIGINATED_FROM|SUNG_IN|HAS_ENERGY_LEVEL|HAS_VALENCE]->(feature)
         MATCH (similar:Song)-[:HAS_GENRE|ORIGINATED_FROM|SUNG_IN|HAS_ENERGY_LEVEL|HAS_VALENCE]->(feature)
         WHERE similar <> seed
         WITH similar, count(feature) as shared_features
         WHERE shared_features >= $min_shared_features
         RETURN similar.id as song_id, shared_features
         ORDER BY shared_features DESC
         LIMIT 20"
    );
    query = query.param("seed_song_id", seed_song_id.as_str()).param("min_shared_features", min_shared_features);
    
    let mut result = app_state.db.execute(query).await
        .map_err(|e| ServerFnError::new(format!("Neo4j query failed: {}", e)))?;
    
    let mut song_ids = Vec::new();
    while let Ok(Some(row)) = result.next().await {
        if let Ok(song_id) = row.get::<String>("song_id") {
            song_ids.push(song_id);
        }
    }
    
    tracing::info!("Found {} similar songs to {} with {} shared features", song_ids.len(), seed_song_id, min_shared_features);
    Ok(song_ids)
}

/// Get available genres from the graph
#[server(GetAvailableGenres)]
pub async fn get_available_genres() -> Result<Vec<String>, ServerFnError> {
    let _user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let query = query("MATCH (g:Genre) RETURN DISTINCT g.name as name ORDER BY g.name");
    
    let mut result = app_state.db.execute(query).await
        .map_err(|e| ServerFnError::new(format!("Neo4j query failed: {}", e)))?;
    
    let mut genres = Vec::new();
    while let Ok(Some(row)) = result.next().await {
        if let Ok(genre_name) = row.get::<String>("name") {
            genres.push(genre_name);
        }
    }
    
    Ok(genres)
}

/// Get available languages from the graph
#[server(GetAvailableLanguages)]
pub async fn get_available_languages() -> Result<Vec<String>, ServerFnError> {
    let _user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let query = query("MATCH (l:Language) RETURN DISTINCT l.name as name ORDER BY l.name");
    
    let mut result = app_state.db.execute(query).await
        .map_err(|e| ServerFnError::new(format!("Neo4j query failed: {}", e)))?;
    
    let mut languages = Vec::new();
    while let Ok(Some(row)) = result.next().await {
        if let Ok(language_name) = row.get::<String>("name") {
            languages.push(language_name);
        }
    }
    
    Ok(languages)
}

/// Enrich playlist songs from database with LLM-powered semantic features
#[server(EnrichPlaylistFromDatabase)]
pub async fn enrich_playlist_from_database(playlist_id: String, song_count: u32) -> Result<String, ServerFnError> {
    let user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    tracing::info!("Starting enrichment for playlist {} for user {}", playlist_id, user.spotify_id);
    
    // 1. Get playlist details from database
    let mut playlist_query = query(
        "MATCH (u:User {spotify_id: $user_id})-[:OWNS]->(p:Playlist {id: $playlist_id})
         RETURN p.id as id, p.name as name, p.description as description"
    );
    playlist_query = playlist_query
        .param("user_id", user.spotify_id.clone())
        .param("playlist_id", playlist_id.clone());
    
    let mut playlist_result = app_state.db.execute(playlist_query).await
        .map_err(|e| ServerFnError::new(format!("Failed to query playlist: {}", e)))?;
    
    let playlist_row = playlist_result.next().await
        .map_err(|e| ServerFnError::new(format!("Failed to read playlist data: {}", e)))?
        .ok_or_else(|| ServerFnError::new("Playlist not found in database"))?;
    
    let playlist_name: String = playlist_row.get("name").unwrap_or_default();
    tracing::info!("Enriching playlist: {}", playlist_name);
    
    // 2. Get non-enriched songs from the playlist (limit to 10)
    let mut songs_query = query(
        "MATCH (u:User {spotify_id: $user_id})-[:OWNS]->(p:Playlist {id: $playlist_id})-[:CONTAINS]->(s:Song)
         MATCH (s)-[:PERFORMED_BY]->(a:Artist)
         WHERE s.is_enriched = false OR s.is_enriched IS NULL
         WITH s, COLLECT(a.name) as artists
         RETURN s.id as id, s.title as title, s.uri as uri, artists
         LIMIT $song_count"
    );
    songs_query = songs_query
        .param("user_id", user.spotify_id)
        .param("playlist_id", playlist_id.clone())
        .param("song_count", song_count as i64);
    
    let mut songs_result = app_state.db.execute(songs_query).await
        .map_err(|e| ServerFnError::new(format!("Failed to query songs: {}", e)))?;
    
    // 3. Build SpotifyTrackItem objects from database data
    let mut tracks = Vec::new();
    loop {
        match songs_result.next().await {
            Ok(Some(row)) => {
                let song_id: String = row.get("id").unwrap_or_default();
                let title: String = row.get("title").unwrap_or_default();
                let uri: String = row.get("uri").unwrap_or_default();
                let artists_names: Vec<String> = row.get("artists").unwrap_or_default();
                
                // Convert to SpotifyTrackItem format for LLM processing
                let artists = artists_names.into_iter().map(|name| crate::api_models::SpotifyArtistSimple {
                    id: None, // We don't need IDs for LLM processing
                    name,
                }).collect();
                
                let track = crate::api_models::SpotifyTrackItem {
                    id: song_id,
                    uri,
                    name: title,
                    artists,
                    album: None,
                    duration_ms: None,
                    explicit: None,
                    audio_features: None,
                };
                
                tracks.push(track);
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    
    if tracks.is_empty() {
        return Ok(format!("🎵 Playlist '{}' has no songs that need enrichment.\nAll songs may already be enriched!", playlist_name));
    }
    
    tracing::info!("Found {} non-enriched songs to process", tracks.len());
    
    // 4. Call LLM to get features with progress tracking
    tracing::info!("🤖 Starting LLM analysis for {} songs...", tracks.len());
    let llm_features = match get_llm_song_features(tracks.clone()).await {
        Ok(features) => features,
        Err(e) => {
            return Ok(format!("❌ LLM feature extraction failed: {}\n\nThis could be due to:\n- Missing OPENROUTER_API_KEY\n- API rate limits\n- Network issues\n\nCheck the server logs for more details.", e));
        }
    };
    
    // 5. Store features in Neo4j graph
    let mut successful_enrichments = 0;
    for feature in &llm_features {
        match store_song_features_in_graph(&feature.song_id, feature).await {
            Ok(_) => {
                // Mark song as enriched in database
                let mut update_query = query("MATCH (s:Song {id: $song_id}) SET s.is_enriched = true");
                update_query = update_query.param("song_id", feature.song_id.as_str());
                
                if let Err(e) = app_state.db.run(update_query).await {
                    tracing::error!("Failed to mark song {} as enriched: {}", feature.song_id, e);
                } else {
                    successful_enrichments += 1;
                }
            }
            Err(e) => {
                tracing::error!("Failed to store features for {}: {}", feature.song_id, e);
            }
        }
    }
    
    // 6. Create detailed report
    let mut report = format!("🎵 Music Library Enrichment for '{}'\n", playlist_name);
    report.push_str("🤖 OpenRouter API + Neo4j Graph Database\n");
    report.push_str(&format!("📊 Successfully enriched {}/{} songs\n\n", successful_enrichments, llm_features.len()));
    
    for (i, (track, features)) in tracks.iter().zip(llm_features.iter()).enumerate() {
        report.push_str(&format!("{}. {} - {}\n", i + 1, track.name, track.primary_artist()));
        report.push_str(&format!("   📈 Confidence: {} ({})\n", 
            features.metadata_confidence.level,
            features.metadata_confidence.explanation
        ));
        
        // Show genres with primary marked
        if !features.genres.is_empty() {
            report.push_str("   🎼 Genres: ");
            let genre_strs: Vec<String> = features.genres.iter().map(|g| {
                if g.is_primary {
                    format!("{}* ({:.1})", g.name, g.confidence)
                } else {
                    format!("{} ({:.1})", g.name, g.confidence)
                }
            }).collect();
            report.push_str(&genre_strs.join(", "));
            report.push_str("\n");
        }
        
        // Show cultural origins
        if !features.cultural_origins.is_empty() {
            report.push_str("   🌍 Cultural Origins: ");
            let culture_strs: Vec<String> = features.cultural_origins.iter()
                .map(|c| format!("{} ({:.1})", c.name, c.confidence))
                .collect();
            report.push_str(&culture_strs.join(", "));
            report.push_str("\n");
        }
        
        // Show language
        if let Some(language) = &features.primary_language {
            report.push_str(&format!("   🗣️  Language: {}\n", language));
        }
        
        // Show energy and valence
        if let Some(energy) = &features.energy_level {
            report.push_str(&format!("   ⚡ Energy: {}", energy));
            if let Some(valence) = &features.valence {
                report.push_str(&format!(", Valence: {}", valence));
            }
            report.push_str("\n");
        }
        
        // Show instruments
        if !features.instruments.is_empty() {
            report.push_str(&format!("   🎸 Instruments: {}\n", features.instruments.join(", ")));
        }
        
        report.push_str("\n");
    }
    
    report.push_str("✅ Enrichment complete! Songs now have semantic graph relationships.");
    
    tracing::info!("Completed enrichment for playlist {}: {}/{} songs processed", 
                   playlist_id, successful_enrichments, llm_features.len());
    Ok(report)
}

/// Test function: Fetch playlist tracks and their audio features (DEPRECATED - use EnrichPlaylistFromDatabase)
#[server(TestPlaylistAudioFeatures)]
pub async fn test_playlist_audio_features(playlist_id: String) -> Result<String, ServerFnError> {
    tracing::info!("Testing LLM audio features for playlist: {}", playlist_id);
    
    // 1. Get playlist details
    let playlist = get_spotify_playlist(playlist_id.clone()).await?;
    tracing::info!("Playlist: {} - {}", playlist.name, playlist.id);
    
    // 2. Get all tracks from the playlist
    let tracks = get_spotify_playlist_tracks_all(playlist_id).await?;
    tracing::info!("Found {} tracks in playlist", tracks.len());
    
    if tracks.is_empty() {
        return Ok(format!("Playlist '{}' is empty", playlist.name));
    }
    
    // 3. Limit to first 3 tracks for LLM testing (to manage costs and response times)
    let test_tracks: Vec<SpotifyTrackItem> = tracks.into_iter().take(3).collect();
    
    tracing::info!("Getting LLM audio features for {} tracks", test_tracks.len());
    
    // 4. Call LLM to get features
    let llm_features = match get_llm_song_features(test_tracks.clone()).await {
        Ok(features) => features,
        Err(e) => {
            return Ok(format!("❌ LLM feature extraction failed: {}\n\nThis could be due to:\n- Missing OPENROUTER_API_KEY\n- API rate limits\n- Network issues\n\nCheck the server logs for more details.", e));
        }
    };
    
    // 5. Store features in Neo4j graph
    for feature in &llm_features {
        if let Err(e) = store_song_features_in_graph(&feature.song_id, feature).await {
            tracing::error!("Failed to store features for {}: {}", feature.song_id, e);
            // Continue with other features even if one fails
        }
    }
    
    // 6. Create detailed report showing graph relationships
    let mut report = format!("🎵 LLM Audio Features Analysis for '{}'\n", playlist.name);
    report.push_str("🤖 OpenRouter API\n");
    report.push_str(&format!("📊 Analyzed {} tracks and stored in Neo4j graph\n\n", llm_features.len()));
    
    for (i, (track, features)) in test_tracks.iter().zip(llm_features.iter()).enumerate() {
        report.push_str(&format!("{}. {} - {}\n", i + 1, track.name, track.primary_artist()));
        report.push_str(&format!("   📈 Confidence: {} ({})\n", 
            features.metadata_confidence.level,
            features.metadata_confidence.explanation
        ));
        
        // Show genres with primary marked
        if !features.genres.is_empty() {
            report.push_str("   🎼 Genres: ");
            let genre_strs: Vec<String> = features.genres.iter().map(|g| {
                if g.is_primary {
                    format!("{}* ({:.1})", g.name, g.confidence)
                } else {
                    format!("{} ({:.1})", g.name, g.confidence)
                }
            }).collect();
            report.push_str(&genre_strs.join(", "));
            report.push_str("\n");
        }
        
        // Show cultural origins
        if !features.cultural_origins.is_empty() {
            report.push_str("   🌍 Cultural Origins: ");
            let culture_strs: Vec<String> = features.cultural_origins.iter()
                .map(|c| format!("{} ({:.1})", c.name, c.confidence))
                .collect();
            report.push_str(&culture_strs.join(", "));
            report.push_str("\n");
        }
        
        // Show language
        if let Some(language) = &features.primary_language {
            report.push_str(&format!("   🗣️  Language: {}\n", language));
        }
        
        // Show energy and valence
        if let Some(energy) = &features.energy_level {
            report.push_str(&format!("   ⚡ Energy: {}", energy));
            if let Some(valence) = &features.valence {
                report.push_str(&format!(", Valence: {}", valence));
            }
            report.push_str("\n");
        }
        
        // Show instruments
        if !features.instruments.is_empty() {
            report.push_str(&format!("   🎸 Instruments: {}\n", features.instruments.join(", ")));
        }
        
        // Show production and vocal style
        if let Some(production) = &features.production_style {
            report.push_str(&format!("   🎛️  Production: {}", production));
        }
        if let Some(vocal) = &features.vocal_presence {
            report.push_str(&format!(" | 🎤 Vocals: {}", vocal));
        }
        if features.production_style.is_some() || features.vocal_presence.is_some() {
            report.push_str("\n");
        }
        
        // Show user tags
        if !features.user_facing_tags.is_empty() {
            report.push_str("   🏷️  Tags: ");
            let tag_strs: Vec<String> = features.user_facing_tags.iter()
                .map(|t| format!("{} ({:.1})", t.tag, t.confidence))
                .collect();
            report.push_str(&tag_strs.join(", "));
            report.push_str("\n");
        }
        
        report.push_str("\n");
    }
    
    // 7. Show graph query examples
    report.push_str("🔍 Example Graph Queries Available:\n");
    report.push_str("   • Find songs by language: /api/find_songs_by_language\n");
    report.push_str("   • Find songs by genre: /api/find_songs_by_genre\n");
    report.push_str("   • Find similar songs: /api/find_similar_songs\n");
    report.push_str("   • Available genres: /api/get_available_genres\n");
    report.push_str("   • Available languages: /api/get_available_languages\n\n");
    
    report.push_str("💾 All features stored as Neo4j graph relationships for fast querying!\n");
    
    tracing::info!("LLM audio features test completed successfully");
    Ok(report)
}

/// Get all available node types and their values for playlist generation
#[server(GetAvailableNodeTypes)]
pub async fn get_available_node_types() -> Result<Vec<NodeTypeInfo>, ServerFnError> {
    let _user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    let mut node_types = Vec::new();
    
    // Query for each node type
    let node_queries = vec![
        ("Genre", "MATCH (n:Genre)<-[r:HAS_GENRE]-(s:Song) RETURN n.name as value, count(s) as song_count, avg(r.confidence) as confidence_avg"),
        ("Language", "MATCH (n:Language)<-[:SUNG_IN]-(s:Song) RETURN n.name as value, count(s) as song_count, null as confidence_avg"),
        ("EnergyLevel", "MATCH (n:EnergyLevel)<-[:HAS_ENERGY_LEVEL]-(s:Song) RETURN n.level as value, count(s) as song_count, null as confidence_avg"),
        ("Valence", "MATCH (n:Valence)<-[:HAS_VALENCE]-(s:Song) RETURN n.level as value, count(s) as song_count, null as confidence_avg"),
        ("Culture", "MATCH (n:Culture)<-[r:ORIGINATED_FROM]-(s:Song) RETURN n.name as value, count(s) as song_count, avg(r.confidence) as confidence_avg"),
        ("Instrument", "MATCH (n:Instrument)<-[:FEATURES_INSTRUMENT]-(s:Song) RETURN n.name as value, count(s) as song_count, null as confidence_avg"),
        ("ProductionStyle", "MATCH (n:ProductionStyle)<-[:HAS_PRODUCTION_STYLE]-(s:Song) RETURN n.name as value, count(s) as song_count, null as confidence_avg"),
        ("VocalStyle", "MATCH (n:VocalStyle)<-[:HAS_VOCAL_STYLE]-(s:Song) RETURN n.name as value, count(s) as song_count, null as confidence_avg"),
        ("UserTag", "MATCH (n:UserTag)<-[r:HAS_USER_TAG]-(s:Song) RETURN n.name as value, count(s) as song_count, avg(r.confidence) as confidence_avg"),
    ];
    
    for (node_type, cypher_query) in node_queries {
        let query = query(cypher_query);
        
        match app_state.db.execute(query).await {
            Ok(mut result) => {
                let mut values = Vec::new();
                
                while let Ok(Some(row)) = result.next().await {
                    let value: String = row.get("value").unwrap_or_default();
                    let song_count: i64 = row.get("song_count").unwrap_or(0);
                    let confidence_avg: Option<f64> = row.get("confidence_avg").ok();
                    
                    if !value.is_empty() && song_count > 0 {
                        values.push(NodeValue {
                            value,
                            song_count,
                            confidence_avg,
                        });
                    }
                }
                
                // Sort by song count descending
                values.sort_by(|a, b| b.song_count.cmp(&a.song_count));
                
                if !values.is_empty() {
                    node_types.push(NodeTypeInfo {
                        node_type: node_type.to_string(),
                        available_values: values,
                    });
                }
            }
            Err(e) => {
                tracing::error!("Failed to query {} nodes: {}", node_type, e);
            }
        }
    }
    
    tracing::info!("Found {} node types with values", node_types.len());
    Ok(node_types)
}

/// Generate a new Spotify playlist based on semantic filters
#[server(GenerateSemanticPlaylist)]
pub async fn generate_semantic_playlist(request: PlaylistGenerationRequest) -> Result<GeneratedPlaylistResult, ServerFnError> {
    let user = require_auth().await?;
    let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await?;
    
    tracing::info!("Generating playlist '{}' with {} filters for user {}", 
                   request.name, request.filters.len(), user.spotify_id);
    
    if request.filters.is_empty() {
        return Err(ServerFnError::new("At least one filter is required".to_string()));
    }
    
    // Build dynamic Cypher query based on filters
    let mut where_clauses = Vec::new();
    let mut match_clauses = Vec::new();
    let mut filter_descriptions = Vec::new();
    
    for (i, filter) in request.filters.iter().enumerate() {
        let var_name = format!("n{}", i);
        
        match filter.node_type.as_str() {
            "Genre" => {
                match_clauses.push(format!("MATCH (s)-[r{}:HAS_GENRE]->({var_name}:Genre {{name: $filter_{i}_value}})", i));
                if let Some(min_conf) = filter.min_confidence {
                    where_clauses.push(format!("r{}.confidence >= $filter_{i}_confidence", i));
                }
                filter_descriptions.push(format!("Genre: {}", filter.value));
            },
            "Language" => {
                match_clauses.push(format!("MATCH (s)-[:SUNG_IN]->({var_name}:Language {{name: $filter_{i}_value}})"));
                filter_descriptions.push(format!("Language: {}", filter.value));
            },
            "EnergyLevel" => {
                match_clauses.push(format!("MATCH (s)-[:HAS_ENERGY_LEVEL]->({var_name}:EnergyLevel {{level: $filter_{i}_value}})"));
                filter_descriptions.push(format!("Energy: {}", filter.value));
            },
            "Valence" => {
                match_clauses.push(format!("MATCH (s)-[:HAS_VALENCE]->({var_name}:Valence {{level: $filter_{i}_value}})"));
                filter_descriptions.push(format!("Valence: {}", filter.value));
            },
            "Culture" => {
                match_clauses.push(format!("MATCH (s)-[r{}:ORIGINATED_FROM]->({var_name}:Culture {{name: $filter_{i}_value}})", i));
                if let Some(min_conf) = filter.min_confidence {
                    where_clauses.push(format!("r{}.confidence >= $filter_{i}_confidence", i));
                }
                filter_descriptions.push(format!("Culture: {}", filter.value));
            },
            "Instrument" => {
                match_clauses.push(format!("MATCH (s)-[:FEATURES_INSTRUMENT]->({var_name}:Instrument {{name: $filter_{i}_value}})"));
                filter_descriptions.push(format!("Instrument: {}", filter.value));
            },
            "ProductionStyle" => {
                match_clauses.push(format!("MATCH (s)-[:HAS_PRODUCTION_STYLE]->({var_name}:ProductionStyle {{name: $filter_{i}_value}})"));
                filter_descriptions.push(format!("Production: {}", filter.value));
            },
            "VocalStyle" => {
                match_clauses.push(format!("MATCH (s)-[:HAS_VOCAL_STYLE]->({var_name}:VocalStyle {{name: $filter_{i}_value}})"));
                filter_descriptions.push(format!("Vocals: {}", filter.value));
            },
            "UserTag" => {
                match_clauses.push(format!("MATCH (s)-[r{}:HAS_USER_TAG]->({var_name}:UserTag {{name: $filter_{i}_value}})", i));
                if let Some(min_conf) = filter.min_confidence {
                    where_clauses.push(format!("r{}.confidence >= $filter_{i}_confidence", i));
                }
                filter_descriptions.push(format!("Tag: {}", filter.value));
            },
            _ => return Err(ServerFnError::new(format!("Unsupported node type: {}", filter.node_type))),
        }
    }
    
    // Ensure songs belong to user's imported playlists
    let user_constraint = "MATCH (u:User {spotify_id: $user_id})-[:OWNS]->(p:Playlist)-[:CONTAINS]->(s:Song)";
    
    // Build the complete query
    let mut cypher_query = format!("{}\n{}", user_constraint, match_clauses.join("\n"));
    if !where_clauses.is_empty() {
        cypher_query.push_str(&format!("\nWHERE {}", where_clauses.join(" AND ")));
    }
    
    let order_clause = if request.shuffle {
        "ORDER BY rand()"
    } else {
        "ORDER BY s.title"
    };
    
    cypher_query.push_str(&format!(
        "\nRETURN DISTINCT s.id as song_id, s.title as title, s.uri as uri\n{}\nLIMIT $max_songs",
        order_clause
    ));
    
    tracing::info!("Generated Cypher query: {}", cypher_query);
    
    // Execute the query
    let mut query_obj = query(&cypher_query);
    query_obj = query_obj.param("user_id", user.spotify_id.clone());
    query_obj = query_obj.param("max_songs", request.max_songs as i64);
    
    // Add filter parameters
    for (i, filter) in request.filters.iter().enumerate() {
        query_obj = query_obj.param(&format!("filter_{}_value", i), filter.value.as_str());
        if let Some(min_conf) = filter.min_confidence {
            query_obj = query_obj.param(&format!("filter_{}_confidence", i), min_conf);
        }
    }
    
    let mut result = app_state.db.execute(query_obj).await
        .map_err(|e| ServerFnError::new(format!("Failed to execute playlist query: {}", e)))?;
    
    // Collect song URIs
    let mut song_uris = Vec::new();
    while let Ok(Some(row)) = result.next().await {
        let uri: String = row.get("uri").unwrap_or_default();
        if !uri.is_empty() {
            song_uris.push(uri);
        }
    }
    
    if song_uris.is_empty() {
        return Err(ServerFnError::new("No songs found matching the specified filters".to_string()));
    }
    
    tracing::info!("Found {} songs matching filters", song_uris.len());
    
    // Create Spotify playlist
    let user_id = get_spotify_user_id().await?;
    let description = request.description.unwrap_or_else(|| {
        format!("Generated by Betterd Spotify with filters: {}", filter_descriptions.join(", "))
    });
    
    let created_playlist_data: SpotifyPlaylistItem = spotify_api_call(|access_token| {
        let user_id = user_id.clone();
        let playlist_name = request.name.clone();
        let description = description.clone();
        async move {
            #[derive(serde::Serialize)]
            struct CreatePlaylistPayload {
                name: String,
                public: bool,
                description: String,
            }
            
            let create_payload = CreatePlaylistPayload {
                name: playlist_name.clone(),
                public: false,
                description,
            };
            
            let create_playlist_url = format!("https://api.spotify.com/v1/users/{}/playlists", user_id);
            let client = Client::new();
            
            match client
                .post(&create_playlist_url)
                .bearer_auth(access_token)
                .json(&create_payload)
                .send().await 
            {
                Ok(response) => {
                    if response.status().is_success() || response.status().as_u16() == 201 {
                        match response.json::<SpotifyPlaylistItem>().await {
                            Ok(data) => Ok(data),
                            Err(e) => Err((500, format!("Failed to parse playlist response: {}", e))),
                        }
                    } else {
                        let status = response.status().as_u16();
                        let error_text = response.text().await.unwrap_or_default();
                        Err((status, format!("Spotify error creating playlist: {}", error_text)))
                    }
                }
                Err(e) => Err((500, format!("Network error creating playlist: {}", e))),
            }
        }
    }).await?;
    
    let playlist_id = created_playlist_data.id.clone();
    tracing::info!("Created Spotify playlist '{}' (ID: {})", request.name, playlist_id);
    
    // Add songs to playlist in batches
    for chunk_uris in song_uris.chunks(100) {
        let chunk_vec: Vec<String> = chunk_uris.to_vec();
        spotify_api_call(|access_token| {
            let playlist_id = playlist_id.clone();
            let chunk_vec = chunk_vec.clone();
            async move {
                #[derive(serde::Serialize)]
                struct AddTracksPayload {
                    uris: Vec<String>,
                }
                
                let add_payload = AddTracksPayload {
                    uris: chunk_vec.clone(),
                };
                
                let add_tracks_url = format!("https://api.spotify.com/v1/playlists/{}/tracks", playlist_id);
                let client = Client::new();
                
                match client.post(&add_tracks_url)
                    .bearer_auth(access_token)
                    .json(&add_payload)
                    .send().await 
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            Ok(())
                        } else {
                            let status = response.status().as_u16();
                            let error_text = response.text().await.unwrap_or_default();
                            Err((status, format!("Error adding tracks: {}", error_text)))
                        }
                    }
                    Err(e) => Err((500, format!("Network error adding tracks: {}", e))),
                }
            }
        }).await?;
        
        // Rate limiting
        if song_uris.len() > 100 && chunk_uris.len() == 100 {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        }
    }
    
    let external_url = format!("https://open.spotify.com/playlist/{}", playlist_id);
    
    let result = GeneratedPlaylistResult {
        spotify_playlist_id: playlist_id,
        name: request.name,
        song_count: song_uris.len() as i64,
        applied_filters: filter_descriptions,
        external_url,
    };
    
    tracing::info!("Successfully generated playlist with {} songs", result.song_count);
    Ok(result)
}
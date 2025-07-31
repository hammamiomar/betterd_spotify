pub mod pkce{
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    use sha2::{Digest,Sha256};

    pub fn generate_code_verifier() -> String{
        let mut rng = thread_rng();
        let verifier : String = std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(128)
            .collect();
        verifier
    }

    pub fn generate_code_challenge(verifier: &str) -> String{
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        URL_SAFE_NO_PAD.encode(hash)
    }
}

/// Authentication helpers for server functions
#[cfg(feature = "server")]
pub mod helpers {
    use dioxus::prelude::*;
    use neo4rs::query;
    use crate::server::AppState;
    use base64::Engine;
    
    /// Simple struct to hold authenticated user info
    pub struct AuthenticatedUser {
        pub spotify_id: String,
        pub access_token: String,
        pub refresh_token: Option<String>,
    }
    
    /// Get the current authenticated user from cookies
    /// Returns None if not authenticated
    pub async fn get_current_user() -> Option<AuthenticatedUser> {
        // Extract cookie from request headers
        let context = dioxus::prelude::server_context();
        let parts = context.request_parts();
        let cookie_header = parts.headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())?;
        
        // Find the session cookie
        let session_id = cookie_header
            .split(';')
            .find(|c| c.trim().starts_with("sid="))
            .map(|c| c.trim().trim_start_matches("sid=").to_string())?;
        
        // Get app state
        let FromContext(app_state) = extract::<FromContext<AppState>, ()>().await.ok()?;
        
        // Query the database for the session
        let mut query = query(
            "MATCH (u:User)-[:HAS_SESSION]->(s:Session {session_id: $sid})
             WHERE s.expires_at > datetime()
             RETURN u.spotify_id AS spotify_id, 
                    u.access_token AS access_token,
                    u.refresh_token AS refresh_token"
        );
        query = query.param("sid", session_id);
        
        let mut result = app_state.db.execute(query).await.ok()?;
        let row = result.next().await.ok()??;
        
        Some(AuthenticatedUser {
            spotify_id: row.get("spotify_id").ok()?,
            access_token: row.get("access_token").ok()?,
            refresh_token: row.get("refresh_token").ok(),
        })
    }
    
    /// Refresh an expired access token using the refresh token
    pub async fn refresh_access_token(user: &AuthenticatedUser) -> Result<AuthenticatedUser, ServerFnError> {
        let refresh_token = match user.refresh_token.as_ref() {
            Some(token) => token,
            None => return Err(ServerFnError::ServerError("No refresh token available".to_string())),
        };
        
        let client_id = match std::env::var("SPOTIFY_CLIENT_ID") {
            Ok(id) => id,
            Err(_) => return Err(ServerFnError::ServerError("SPOTIFY_CLIENT_ID not set".to_string())),
        };
        let client_secret = match std::env::var("SPOTIFY_CLIENT_SECRET") {
            Ok(secret) => secret,
            Err(_) => return Err(ServerFnError::ServerError("SPOTIFY_CLIENT_SECRET not set".to_string())),
        };
        
        let client = reqwest::Client::new();
        let token_endpoint = "https://accounts.spotify.com/api/token";
        
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];
        
        let auth_header = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", client_id, client_secret));
        
        tracing::info!("Refreshing access token for user {}", user.spotify_id);
        
        let response = match client
            .post(token_endpoint)
            .header("Authorization", format!("Basic {}", auth_header))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await {
                Ok(resp) => resp,
                Err(e) => return Err(ServerFnError::ServerError(format!("Network error refreshing token: {}", e))),
            };
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Token refresh failed: {} - {}", status, error_text);
            return Err(ServerFnError::ServerError(format!("Token refresh failed: {}", status)));
        }
        
        #[derive(serde::Deserialize)]
        struct TokenRefreshResponse {
            access_token: String,
            refresh_token: Option<String>,
        }
        
        let token_response: TokenRefreshResponse = match response.json().await {
            Ok(data) => data,
            Err(e) => return Err(ServerFnError::ServerError(format!("Failed to parse refresh response: {}", e))),
        };
        
        // Update the database with the new tokens
        let FromContext(app_state) = match extract::<FromContext<AppState>, ()>().await {
            Ok(state) => state,
            Err(e) => return Err(ServerFnError::ServerError(format!("Failed to get app state: {}", e))),
        };
        
        let mut update_query = query(
            "MATCH (u:User {spotify_id: $id})
             SET u.access_token = $access_token,
                 u.refresh_token = COALESCE($refresh_token, u.refresh_token),
                 u.token_updated_at = datetime()"
        );
        update_query = update_query
            .param("id", user.spotify_id.clone())
            .param("access_token", token_response.access_token.clone())
            .param("refresh_token", token_response.refresh_token.clone().unwrap_or_else(|| refresh_token.clone()));
        
        if let Err(e) = app_state.db.run(update_query).await {
            tracing::error!("Failed to update tokens in database: {}", e);
            return Err(ServerFnError::ServerError("Failed to update tokens".to_string()));
        }
        
        tracing::info!("Successfully refreshed access token for user {}", user.spotify_id);
        
        Ok(AuthenticatedUser {
            spotify_id: user.spotify_id.clone(),
            access_token: token_response.access_token,
            refresh_token: Some(token_response.refresh_token.unwrap_or_else(|| refresh_token.clone())),
        })
    }
    
    /// Get a valid access token, refreshing if necessary
    pub async fn get_valid_access_token() -> Result<String, ServerFnError> {
        let user = require_auth().await?;
        Ok(user.access_token)
    }
    
    /// Handle 401 errors by refreshing the token and returning a new one
    pub async fn handle_token_expired_error() -> Result<String, ServerFnError> {
        let user = require_auth().await?;
        let refreshed_user = refresh_access_token(&user).await?;
        Ok(refreshed_user.access_token)
    }
    
    /// Require authentication with automatic token refresh
    pub async fn require_auth_with_refresh() -> Result<AuthenticatedUser, ServerFnError> {
        match get_current_user().await {
            Some(user) => Ok(user),
            None => Err(ServerFnError::ServerError("Authentication required".to_string())),
        }
    }
    
    /// Require authentication - returns error if not authenticated  
    pub async fn require_auth() -> Result<AuthenticatedUser, ServerFnError> {
        get_current_user().await
            .ok_or_else(|| ServerFnError::ServerError("Authentication required".to_string()))
    }

    /// Generic Spotify API call wrapper with automatic token refresh
    pub async fn spotify_api_call<F, Fut, T>(
        api_call: F,
    ) -> Result<T, ServerFnError>
    where
        F: Fn(String) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<T, (u16, String)>>,
    {
        let mut access_token = get_valid_access_token().await?;
        let mut retry_count = 0;
        
        loop {
            match api_call(access_token.clone()).await {
                Ok(result) => return Ok(result),
                Err((status_code, error_msg)) => {
                    if status_code == 401 && retry_count == 0 {
                        tracing::info!("Access token expired, attempting refresh");
                        retry_count += 1;
                        access_token = handle_token_expired_error().await?;
                        continue;
                    } else {
                        return Err(ServerFnError::ServerError(format!(
                            "Spotify API Error ({}): {}", status_code, error_msg
                        )));
                    }
                }
            }
        }
    }
}
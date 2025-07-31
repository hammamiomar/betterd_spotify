use std::{env, sync::RwLock};
use anyhow::Result;
use axum::{response::Redirect, routing::get, extract::State};
use dioxus::prelude::*;
use dioxus_logger;
use dotenvy::dotenv;
use neo4rs::Graph;
use std::{collections::HashMap, sync::{Mutex,Arc}};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use axum_extra::extract::cookie::*;
use uuid::Uuid;
use chrono::{ Utc,Duration};
use cookie::time::Duration as CookieDuration;


use serde::Deserialize;

use crate::{api_models::{SpotifyTokenResponse,SpotifyUserProfile}, App};
use crate::auth::pkce;


#[derive(Clone)]
pub struct AppState{
    pub pkce_verifiers : Arc<Mutex<HashMap<String, String>>>,
    pub db: Arc<Graph>
}

#[cfg(feature = "server")]
pub async fn start_server() -> Result<()> {
    use std::any::Any;


    dotenv().ok();

    dioxus_logger::init(tracing::Level::INFO).expect("failed to init logger");

    let _client_id = env::var("SPOTIFY_CLIENT_ID")
        .expect("SPOTIFY_CLIENT_ID must be set in .env");

    let db_client = crate::db::connect().await;
    let app_state = AppState{
        pkce_verifiers: Arc::new(Mutex::new(HashMap::new())),
        db: db_client,
    };
    
    let provider = {
        let shared = app_state.clone();
        move || Box::new(shared.clone()) as Box<dyn Any>
    };

    let cfg = ServeConfigBuilder::default()
        .context_providers(Arc::new(vec![Box::new(provider)]));

     let address = dioxus::cli_config::fullstack_address_or_localhost();

    let axum_router = axum::Router::new()
        .route("/auth/spotify", get(spotify_login_handler))
        .route("/callback", get(spotify_callback_handler))
        .route("/auth/logout", get(axum_logout_handler)) 
        .with_state(app_state.clone())
        .serve_dioxus_application(cfg,App);

    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, axum_router.into_make_service())
        .await?;

    Ok(())
}

async fn spotify_login_handler(
    State(app_state):State<AppState>,
) -> Redirect{

    let client_id = env::var("SPOTIFY_CLIENT_ID")
        .expect("sptify client id must be set");
    let redirect_uri = env::var("SPOTIFY_REDIRECT_URI")
        .expect("Spotify redirect uri must be set");

    //generate pkce codes
    let code_verifier = pkce::generate_code_verifier();
    let code_challenge = pkce::generate_code_challenge(&code_verifier);

    // generate random state param for CSRF protection
    let state:String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();

    //stroing code verifier temporarily

    app_state
        .pkce_verifiers
        .lock()
        .unwrap()
        .insert(state.clone(), code_verifier);
    
    tracing::info!("Stored verifier for state:{}",state);

    let scope = "playlist-read-private playlist-read-collaborative playlist-modify-private playlist-modify-public
    user-read-private user-read-email ugc-image-upload user-library-read";

    // construct URL

    let mut auth_url = reqwest::Url::parse("https://accounts.spotify.com/authorize").unwrap();
    auth_url.query_pairs_mut()
        .append_pair("response_type","code")
        .append_pair("client_id", &client_id)
        .append_pair("scope", scope)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("state", &state)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &code_challenge);
    tracing::info!("Redirecting user to spotify: {}", auth_url);


    Redirect::temporary(auth_url.as_str())
}


#[axum::debug_handler]
async fn spotify_callback_handler(
    State(app_state):State<AppState>,
    jar: CookieJar,
    query: axum::extract::Query<std::collections::HashMap<String,String>>,
) -> (CookieJar, Redirect){
    // query will either respond with  code and state, or error and state
    
    let code = match query.get("code"){
        Some(c) => c.clone(),
        None => {
            tracing::error!("Callback missing code param");
            return (jar,Redirect::temporary("/login?error=missing_code"));
        }
    };

    let received_state = match query.get("state"){
        Some(s) => s.clone(),
        None => {
            tracing::error!("Callback missing state param");
            return (jar, Redirect::temporary("/login?error=missing_state"));
        }
    };

    // get code verifier and match state, for CSRF protection

    let code_verifier = {
        let mut verifiers = app_state.pkce_verifiers.lock().unwrap();

        match verifiers.remove(&received_state){
            Some(v) => v,
            None => {
                tracing::error!("state mismatch OR verifier not found for state");
                return (jar,Redirect::temporary("/login?error=state_mismatch"))
            }
        }
    };
    tracing::info!("Retrieved verifier for state: {}",received_state);

    // ---------------- Exchange code for access token

    let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID must be set");
    let redirect_uri = env::var("SPOTIFY_REDIRECT_URI").expect("SPOTIFY_REDIRECT_URI must be set");
    let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("SPOTIFY_CLIENT_SECRET must be set");

    // prepare token request

    let client = reqwest::Client::new();
    let token_endpoint = "https://accounts.spotify.com/api/token";

    let params = [
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri",&redirect_uri),
        ("client_id",&client_id),
        ("code_verifier",&code_verifier),
    ];

    let auth_header_value = format!(
        "Basic {}",
        URL_SAFE_NO_PAD.encode(format!("{}:{}", client_id,client_secret))
    );

    tracing::info!("Requesting Access Token");

    let response_result = client
        .post(token_endpoint)
        .header(AUTHORIZATION, auth_header_value)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(ACCEPT, "application/json")
        .form(&params)
        .send()
        .await;

    let tokens = match response_result{
        Ok(token_response) =>{
            if token_response.status().is_success(){
                match token_response.json::<SpotifyTokenResponse>().await{
                    Ok(token) =>{
                        tracing::info!("Succesfully obtained tokens: {:?}", token);

                        token
                    }

                    Err(e) => {
                        tracing::error!( "failed to parse token response json:{}",e);
                        return (jar, Redirect::temporary("/login?error=token_parse_failed"));
                    }
                }
            }else{
                let status = token_response.status();
                let text = token_response.text().await.unwrap_or_else(|_| {
                    "Failed to read error body".to_string()
                });
                tracing::error!("Token request failed with status {}:{}", status,text);
                return (jar, Redirect::temporary("/login?error=token_request_failed"))
            }
        }
        Err(e) =>{
            tracing::error!("Failed to send token request: {}", e);
            return (jar, Redirect::temporary("/login?error=network_error"))
        }
    };

    // Get User Spotify Profile to setup profile
    let user_profile_response = client
        .get("https://api.spotify.com/v1/me")
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .unwrap()
        .json::<SpotifyUserProfile>()
        .await;

    let spotify_user = match user_profile_response {
        Ok(u) => u,
        Err(_) => return (jar, Redirect::temporary("/login?error=profile_fetch_failed")),
    };

    // NEO4J user setup
    let mut user_query = neo4rs::query(
            "MERGE (u:User {spotify_id: $id})
            SET u.display_name = $name,
                u.access_token = $access,
                u.refresh_token = $refresh,
                u.last_login = datetime()",
        );
    user_query = user_query
        .param("id", spotify_user.id.clone())
        .param("name", spotify_user.display_name)
        .param("access", tokens.access_token)
        .param("refresh", tokens.refresh_token.unwrap_or_default()); // Handle optional refresh token

    if let Err(e) = app_state.db.run(user_query).await {
        tracing::error!("Failed to write user to DB: {}", e);
        return (jar, Redirect::temporary("/login?error=db_error"));
    }

    // --- Step 5: Create a Secure Session for the User ---
    let session_id = Uuid::new_v4().to_string();
    let expires = Utc::now() + Duration::days(7);
    let expires_str = expires.to_rfc3339();

    let mut session_query = neo4rs::query(
        "MATCH (u:User {spotify_id: $id})
         CREATE (s:Session {session_id: $sid, expires_at: datetime($exp)})
         CREATE (u)-[:HAS_SESSION]->(s)",
    );
    session_query = session_query
        .param("id", spotify_user.id)
        .param("sid", session_id.clone())
        .param("exp", expires_str);

    if let Err(e) = app_state.db.run(session_query).await {
        tracing::error!("Failed to write session to DB: {}", e);
        return (jar, Redirect::temporary("/login?error=db_error"));
    }

    // --- Step 6: Set the Session Cookie and Redirect to Home ---
    let cookie = Cookie::build(("sid", session_id))
        .path("/")
        .secure(cfg!(not(debug_assertions))) // Only secure in production
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::days(7)) // Cookie expires in 7 days
        .build();

    // Add the cookie to the jar and redirect
    let new_jar = jar.add(cookie);
    (new_jar, Redirect::temporary("/"))
}

async fn axum_logout_handler(jar: CookieJar) -> (CookieJar, Redirect) {
    // Remove the old cookie by creating a new one that expires immediately.
    let cookie = Cookie::build(("sid", ""))
        .path("/")
        .max_age(CookieDuration::ZERO)
        .build();

    // Add the expiring cookie to the jar and redirect to home page.
    (jar.add(cookie), Redirect::to("/"))
}
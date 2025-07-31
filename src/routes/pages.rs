use dioxus::prelude::*;
use crate::api::{get_spotify_user_playlists_all, get_spotify_user_profile, test_playlist_audio_features};
use crate::db_operations::{import_playlist_to_db, get_playlist_import_status};
use crate::api_models::{SpotifyPlaylistItem, SpotifyUserProfile};
use crate::components::spotify::{PlaylistsView, ProfileView};
use crate::Route;

#[component]
pub fn ShufflePage() -> Element{
    let playlists_resource : Resource<Result<Vec<SpotifyPlaylistItem>,ServerFnError>> = use_server_future(|| async{
    get_spotify_user_playlists_all().await})?;
    let mut search_term = use_signal(String::new);
    let selected_playlist : Signal<Option<SpotifyPlaylistItem>> = use_signal(|| None);

    let navigator = use_navigator();

    rsx!{
        div {class:"space-y-8 p-4 md:p-8",
            div { // Welcome section
                    class: "card-glass float p-8 text-center",
                    h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                        " ╔═════════════════════════════╗"
                    }
                    h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                        " ║      PLAYLIST SHUFFLE      ║"
                    }
                    h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                        " ╚═════════════════════════════╝"
                    }
                    p { class: "text-lg font-mono", style: "color: #4f6d44;", "> True random audio sequencing..." }
                    div { class: "mt-4 flex justify-center space-x-1 font-mono",
                        div { class: "animate-pulse", style: "color: #9fc08e;", "(" }
                        div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "~" }
                        div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "~" }
                        div { class: "animate-pulse", style: "color: #9fc08e;", ")" }
                    }
                }
            //Search
            div {
                class: "card-glass p-6",
                label { class: "block text-sm font-medium mb-2 font-mono", style: "color: #4f6d44;", "┌─ SEARCH PLAYLISTS ─┐" }
                input {
                    r#type: "text",
                    placeholder: ">>> Filter playlists...",
                    class: "input-glass w-full font-mono",
                    value: "{search_term}", // Bind input value to the signal
                    oninput: move |evt| search_term.set(evt.value()), // Update signal on input
                }
            }
        // --- Playlists Section ---
            div {
                id: "shuffle-playlist-selection-list",
                class: "card-glass p-6",
                h2 { class: "text-2xl font-semibold text-gradient mb-4 pb-2 font-mono", style: "border-bottom: 1px solid rgba(127, 168, 109, 0.3);", "┌─ PLAYLIST LIBRARY ─┐" }
                {
                    match playlists_resource.read().as_ref() {
                        Some(Ok(all_playlists_vec)) => {
                            // Filter playlists based on search_term
                            let filtered_playlists = {
                                let search_lower = search_term.read().to_lowercase();
                                if search_lower.is_empty() {
                                    all_playlists_vec.clone() // No filter, show all (clone for iteration)
                                } else {
                                    all_playlists_vec.iter().filter(move |p| {
                                        p.name.to_lowercase().contains(&search_lower)
                                    }).cloned().collect::<Vec<SpotifyPlaylistItem>>()
                                }
                            };

                            if filtered_playlists.is_empty() && !search_term.read().is_empty() {
                                rsx! { p { class: "text-center py-8 font-mono", style: "color: #648a54;", "[ NO MATCH ] - Search parameters yielded zero results."}}
                            } else if filtered_playlists.is_empty() {
                                rsx! { p { class: "text-center py-8 font-mono", style: "color: #648a54;", "[ EMPTY ] - No playlists found in library."}}
                            } else {
                                // Pass down the selected_playlist signal and filtered list
                                rsx!{PlaylistsView {
                                    playlists: filtered_playlists,
                                    selected_playlist: selected_playlist // Pass the signal
                                }}
                            }
                        }
                        Some(Err(e)) => {
                            rsx! { p { class: "text-center py-8 font-mono", style: "color: #c1534d;", "[ ERROR ] - System malfunction detected: {e}" } }
                        }
                        None => {rsx! {p { class: "text-center py-8 font-mono", style: "color: #648a54;", "[ LOADING ] - Scanning molecular database..."}}}
                    }
                }
            }
            div {
                class: "mt-8 text-center",
                button {
                    disabled: selected_playlist.read().is_none(),
                    class: if selected_playlist.read().is_some() { "btn-glass text-lg font-semibold sparkle font-mono" } else { "btn-glass text-lg font-semibold opacity-50 cursor-not-allowed font-mono" },
                    onclick: move |_| {
                        if let Some(playlist) = selected_playlist.read().as_ref() {
                            navigator.push(Route::ShuffleActionPage {
                                playlist_id: playlist.id.clone(),
                                playlist_name: playlist.name.clone(),
                            });
                        }
                    },
                    if selected_playlist.read().is_some() {
                        "[ START SHUFFLE ]"
                    } else {
                        "[ SELECT PLAYLIST ]"
                    }
                }
            }

        }
        
    }
}

#[component]
pub fn Home() -> Element {
    let is_authenticated = use_server_future( || async {
        crate::api::check_auth().await})?;

    rsx! {
        div {class: "space-y-8 p-4 md:p-8", 

            div { // Welcome section
                class: "card-glass float p-8 text-center",
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╔══════════════════════════════════╗"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ║         BETTER SPOTIFY           ║"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╚══════════════════════════════════╝"
                }
                p { class: "text-lg font-mono mb-4", style: "color: #4f6d44;", 
                    "> True randomization. No algorithmic bias."
                }
                p { class: "text-md font-mono mb-2", style: "color: #648a54;", 
                    "[ DEFEATING SPOTIFY'S PREFERENCE ALGORITHMS ]"
                }
                p { class: "text-sm font-mono", style: "color: #7fa86d;", 
                    "// Real shuffle for your 6000+ song collections"
                }
                div { class: "mt-6 flex justify-center space-x-1 font-mono",
                    div { class: "animate-pulse", style: "color: #9fc08e;", "[" }
                    div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "▓" }
                    div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "▓" }
                    div { class: "animate-pulse", style: "color: #9fc08e;", "]" }
                }
            }

            // --- User Profile Section ---
            match is_authenticated.read().as_ref() {
                Some(Ok(true)) => rsx! {
                    // Show authenticated content
                    AuthenticatedUserProfile {}
                },
                Some(Ok(false)) => rsx! {
                    div {
                        class: "card-glass p-6 text-center",
                        h2 { class: "text-2xl font-semibold text-gradient mb-4 font-mono", "┌─ ACCESS CONTROL ─┐" }
                        p { class: "font-mono mb-6", style: "color: #4f6d44;", ">>> Authentication required to access shuffle protocols" }
                        Link {
                            to: Route::LoginPage {},
                            class: "btn-glass text-lg font-semibold font-mono",
                            "[ AUTHENTICATE WITH SPOTIFY ]"
                        }
                    }
                },
                _ => rsx! {}  // Loading or error state
            }
        }
    }
}

#[component]
fn AuthenticatedUserProfile() -> Element {
    let profile_resource: Resource<Result<SpotifyUserProfile, ServerFnError>> = use_server_future( || async {
        get_spotify_user_profile().await})?;
    
    rsx! {
        div {
            id: "user-profile",
            class: "card-glass p-6",
            h2 { class: "text-2xl font-semibold text-gradient mb-4 font-mono", "┌─ USER PROFILE ─┐" }
            {
                match profile_resource.read().as_ref() {
                    Some(Ok(profile)) => rsx! { ProfileView { profile: profile.clone() } },
                    Some(Err(e)) => rsx! { p { class: "font-mono", style: "color: #c1534d;", "[ ERROR ] - Profile loading failed: {e}" } },
                    None => rsx! { p { class: "font-mono", style: "color: #648a54;", "[ LOADING ] - Fetching user profile..." } }
                }
            }
        }
    }
}


#[component]
pub fn LoginPage() -> Element {
    rsx! {
       div {
            class: "flex-grow flex flex-col items-center justify-center p-4", // Centers content vertically and horizontally

            div { // The "card" container for login content
                class: "card-glass p-8 md:p-12 max-w-md w-full text-center",

                h1 {
                    class: "text-3xl font-bold text-gradient mb-4 font-mono",
                    "┌─ AUTHENTICATION ─┐"
                }
                p {
                    class: "font-mono mb-8 text-lg",
                    style: "color: #4f6d44;",
                    "> Spotify access required for playlist operations"
                }
                p {
                    class: "font-mono mb-8 text-sm",
                    style: "color: #648a54;",
                    "// Grant permissions to enable true random shuffle"
                }

                a {
                    href: "/auth/spotify", // This path is handled by your Axum server
                    class: "btn-glass text-lg font-semibold font-mono w-full block",
                    "[ CONNECT TO SPOTIFY ]"
                }

                p {
                    class: "text-xs font-mono mt-8",
                    style: "color: #7fa86d;",
                    "* Read-only access to playlists and basic profile data *"
                }
                p {
                    class: "text-xs font-mono",
                    style: "color: #7fa86d;",
                    "* No data persistence beyond session requirements *"
                }
            }
        }
    }
    
}

#[component]
pub fn AudioFeaturesTestPage() -> Element {
    let mut playlist_id = use_signal(String::new);
    let mut test_result = use_signal(|| Option::<String>::None);
    let mut is_testing = use_signal(|| false);

    rsx! {
        div { class: "space-y-8 p-4 md:p-8",
            div { // Header section
                class: "card-glass float p-8 text-center",
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╔═══════════════════════════════╗"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ║     AUDIO FEATURES TEST      ║"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╚═══════════════════════════════╝"
                }
                p { class: "text-lg font-mono", style: "color: #4f6d44;", 
                    "> Test audio feature analysis capabilities..."
                }
                div { class: "mt-4 flex justify-center space-x-1 font-mono",
                    div { class: "animate-pulse", style: "color: #9fc08e;", "[" }
                    div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "■" }
                    div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "■" }
                    div { class: "animate-pulse", style: "color: #9fc08e;", "]" }
                }
            }

            div { // Input section
                class: "card-glass p-6",
                h2 { class: "text-xl font-semibold text-gradient mb-4 font-mono", 
                    "┌─ PLAYLIST IDENTIFIER ─┐" 
                }
                input {
                    r#type: "text",
                    placeholder: ">>> Enter playlist ID (e.g., 37i9dQZF1DXcBWIGoYBM5M)",
                    class: "input-glass w-full mb-4 font-mono",
                    value: "{playlist_id}",
                    oninput: move |evt| playlist_id.set(evt.value()),
                }
                button {
                    disabled: playlist_id.read().is_empty() || *is_testing.read(),
                    class: if playlist_id.read().is_empty() || *is_testing.read() { 
                        "btn-glass text-lg font-semibold opacity-50 cursor-not-allowed font-mono" 
                    } else { 
                        "btn-glass text-lg font-semibold sparkle font-mono" 
                    },
                    onclick: move |_| {
                        let pid = playlist_id.read().clone();
                        if !pid.is_empty() {
                            is_testing.set(true);
                            test_result.set(None);
                            spawn(async move {
                                match test_playlist_audio_features(pid).await {
                                    Ok(report) => test_result.set(Some(report)),
                                    Err(e) => test_result.set(Some(format!("❌ Error: {}", e))),
                                }
                                is_testing.set(false);
                            });
                        }
                    },
                    if *is_testing.read() {
                        "[ ANALYZING... ]"
                    } else {
                        "[ EXECUTE ANALYSIS ]"
                    }
                }
                p { class: "text-sm mt-2 font-mono", style: "color: #648a54;",
                    "// Extract ID from URL: https://open.spotify.com/playlist/ID_HERE"
                }
            }

            // Results section
            if let Some(result) = test_result.read().as_ref() {
                div { class: "card-glass p-6",
                    h2 { class: "text-xl font-semibold text-gradient mb-4 font-mono", 
                        "┌─ ANALYSIS OUTPUT ─┐" 
                    }
                    pre { 
                        class: "p-4 rounded font-mono text-sm overflow-x-auto whitespace-pre-wrap",
                        style: "background: rgba(26, 34, 24, 0.8); color: #c1d4b6; border: 1px solid rgba(127, 168, 109, 0.3);",
                        "{result}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn ImportDataPage() -> Element {
    let playlists_resource: Resource<Result<Vec<SpotifyPlaylistItem>, ServerFnError>> = 
        use_server_future(|| get_spotify_user_playlists_all())?;
    let mut search_term = use_signal(String::new);
    let import_status = use_signal(|| std::collections::HashMap::<String, String>::new());
    let importing_playlist = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "space-y-8 p-4 md:p-8",
            div { // Header section
                class: "card-glass float p-8 text-center",
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╔═══════════════════════════════╗"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ║     DATABASE IMPORT         ║"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╚═══════════════════════════════╝"
                }
                p { class: "text-lg font-mono", style: "color: #4f6d44;", 
                    "> Import playlists into local database..."
                }
                div { class: "mt-4 flex justify-center space-x-1 font-mono",
                    div { class: "animate-pulse", style: "color: #9fc08e;", "{{" }
                    div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "~" }
                    div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "}}" }
                }
            }

            // Search section
            div {
                class: "card-glass p-6",
                label { class: "block text-sm font-medium mb-2 font-mono", style: "color: #4f6d44;", "┌─ FILTER PLAYLISTS ─┐" }
                input {
                    r#type: "text",
                    placeholder: ">>> Filter playlists...",
                    class: "input-glass w-full font-mono",
                    value: "{search_term}",
                    oninput: move |evt| search_term.set(evt.value()),
                }
            }

            // Playlists section
            div {
                class: "card-glass p-6",
                h2 { class: "text-2xl font-semibold text-gradient mb-4 pb-2 font-mono", 
                    style: "border-bottom: 1px solid rgba(127, 168, 109, 0.3);",
                    "┌─ AVAILABLE PLAYLISTS ─┐" 
                }
                
                match playlists_resource.read().as_ref() {
                    Some(Ok(all_playlists)) => {
                        // Filter playlists based on search term
                        let filtered_playlists = {
                            let search_lower = search_term.read().to_lowercase();
                            if search_lower.is_empty() {
                                all_playlists.clone()
                            } else {
                                all_playlists.iter().filter(|p| {
                                    p.name.to_lowercase().contains(&search_lower)
                                }).cloned().collect::<Vec<SpotifyPlaylistItem>>()
                            }
                        };

                        if filtered_playlists.is_empty() && !search_term.read().is_empty() {
                            rsx! { 
                                p { class: "text-center py-8 font-mono", style: "color: #648a54;", 
                                    "[ NO MATCH ] - Search parameters yielded zero results." 
                                }
                            }
                        } else if filtered_playlists.is_empty() {
                            rsx! { 
                                p { class: "text-center py-8 font-mono", style: "color: #648a54;", 
                                    "[ EMPTY ] - No playlists detected in user library." 
                                }
                            }
                        } else {
                            rsx! {
                                div { class: "space-y-4",
                                    for playlist in filtered_playlists {
                                        PlaylistImportCard {
                                            playlist: playlist.clone(),
                                            import_status: import_status,
                                            importing_playlist: importing_playlist,
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(e)) => rsx! { 
                        p { class: "text-center py-8 font-mono", style: "color: #c1534d;", 
                            "[ ERROR ] - System malfunction detected: {e}" 
                        } 
                    },
                    None => rsx! { 
                        p { class: "text-center py-8 font-mono", style: "color: #648a54;", 
                            "[ LOADING ] - Scanning playlist database..." 
                        } 
                    }
                }
            }
        }
    }
}

#[component]
fn PlaylistImportCard(
    playlist: SpotifyPlaylistItem,
    import_status: Signal<std::collections::HashMap<String, String>>,
    importing_playlist: Signal<Option<String>>,
) -> Element {
    let is_importing = importing_playlist.read().as_ref() == Some(&playlist.id);
    let status_message = import_status.read().get(&playlist.id).cloned();

    rsx! {
        div { 
            class: "card-glass p-4 flex items-center justify-between",
            
            // Playlist info
            div { class: "flex items-center space-x-4 flex-1",
                // Playlist image
                if let Some(images) = &playlist.images {
                    if let Some(image) = images.first() {
                        img {
                            src: "{image.url}",
                            alt: "Playlist cover",
                            class: "w-16 h-16 rounded-md object-cover",
                        }
                    } else {
                        div { class: "w-16 h-16 rounded-md flex items-center justify-center font-mono text-xl",
                            style: "background: rgba(100, 138, 84, 0.3); color: #7fa86d;",
                            "♪"
                        }
                    }
                } else {
                    div { class: "w-16 h-16 rounded-md flex items-center justify-center font-mono text-xl",
                        style: "background: rgba(100, 138, 84, 0.3); color: #7fa86d;",
                        "♪"
                    }
                }
                
                // Playlist details
                div { class: "flex-1",
                    h3 { class: "text-lg font-semibold text-gradient mb-1 font-mono",
                        "{playlist.name}"
                    }
                    if let Some(description) = &playlist.description {
                        if !description.is_empty() {
                            p { class: "text-sm mb-1 font-mono", style: "color: #648a54;",
                                "{description}"
                            }
                        }
                    }
                    p { class: "text-xs font-mono", style: "color: #7fa86d;",
                        "// ID: {playlist.id}"
                    }
                }
            }
            
            // Status and action
            div { class: "flex items-center space-x-3",
                // Status message
                if let Some(message) = status_message {
                    div { class: "text-sm font-mono",
                        if message.contains("SUCCESS") || message.contains("✅") {
                            span { style: "color: #7fa86d;", "[ SUCCESS ]" }
                        } else if message.contains("ERROR") || message.contains("❌") {
                            span { style: "color: #c1534d;", "[ ERROR ]" }
                        } else {
                            span { style: "color: #9fc08e;", "[ PROCESSING ]" }
                        }
                    }
                }
                
                // Import button
                button {
                    disabled: is_importing,
                    class: if is_importing {
                        "btn-glass text-sm font-semibold opacity-50 cursor-not-allowed font-mono"
                    } else {
                        "btn-glass text-sm font-semibold font-mono"
                    },
                    onclick: move |_| {
                        let playlist_id = playlist.id.clone();
                        importing_playlist.set(Some(playlist_id.clone()));
                        
                        spawn(async move {
                            match import_playlist_to_db(playlist_id.clone()).await {
                                Ok(result) => {
                                    import_status.write().insert(playlist_id, result);
                                }
                                Err(e) => {
                                    import_status.write().insert(playlist_id, format!("❌ Error: {}", e));
                                }
                            }
                            importing_playlist.set(None);
                        });
                    },
                    if is_importing {
                        "[ IMPORTING... ]"
                    } else {
                        "[ IMPORT TO DB ]"
                    }
                }
            }
        }
    }
}
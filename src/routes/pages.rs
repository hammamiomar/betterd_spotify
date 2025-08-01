use dioxus::prelude::*;
use crate::api::{get_spotify_user_playlists_all, get_spotify_user_profile, test_playlist_audio_features, enrich_playlist_from_database, get_available_node_types, generate_semantic_playlist};
use crate::db_operations::{import_playlist_to_db, get_playlist_import_status, get_user_imported_playlists, get_playlist_enrichment_status};
use crate::api_models::{SpotifyPlaylistItem, SpotifyUserProfile, ImportedPlaylist, NodeTypeInfo, PlaylistGenerationRequest, PlaylistFilter};
use crate::components::spotify::{PlaylistsView, ProfileView};
use crate::Route;

#[cfg(feature = "web")]
use gloo_timers::future::TimeoutFuture;

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
pub fn MusicLibraryEnrichmentPage() -> Element {
    let playlists_resource: Resource<Result<Vec<ImportedPlaylist>, ServerFnError>> = 
        use_server_future(|| get_user_imported_playlists())?;
    let mut search_term = use_signal(String::new);
    let mut filter_status = use_signal(|| "all".to_string()); // "all", "enriched", "not_enriched"
    let enrichment_status = use_signal(|| std::collections::HashMap::<String, String>::new());
    let enriching_playlist = use_signal(|| Option::<String>::None);
    let enrichment_result = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "space-y-8 p-4 md:p-8",
            div { // Header section
                class: "card-glass float p-8 text-center",
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╔═══════════════════════════════╗"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ║  MUSIC LIBRARY ENRICHMENT   ║"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╚═══════════════════════════════╝"
                }
                p { class: "text-lg font-mono", style: "color: #4f6d44;", 
                    "> Enrich your music library with semantic intelligence..."
                }
                div { class: "mt-4 flex justify-center space-x-1 font-mono",
                    div { class: "animate-pulse", style: "color: #9fc08e;", "⟨" }
                    div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "◈" }
                    div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "⟩" }
                }
            }

            // Search and Filter section
            div {
                class: "card-glass p-6",
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    div {
                        label { class: "block text-sm font-medium mb-2 font-mono", style: "color: #4f6d44;", "┌─ SEARCH PLAYLISTS ─┐" }
                        input {
                            r#type: "text",
                            placeholder: ">>> Filter playlists...",
                            class: "input-glass w-full font-mono",
                            value: "{search_term}",
                            oninput: move |evt| search_term.set(evt.value()),
                        }
                    }
                    div {
                        label { class: "block text-sm font-medium mb-2 font-mono", style: "color: #4f6d44;", "┌─ FILTER STATUS ─┐" }
                        select {
                            class: "input-glass w-full font-mono",
                            value: "{filter_status}",
                            onchange: move |evt| filter_status.set(evt.value()),
                            option { value: "all", "All Playlists" }
                            option { value: "not_enriched", "Not Enriched" }
                            option { value: "partially_enriched", "Partially Enriched" }
                            option { value: "fully_enriched", "Fully Enriched" }
                        }
                    }
                }
            }

            // Playlists section
            div {
                class: "card-glass p-6",
                h2 { class: "text-2xl font-semibold text-gradient mb-4 pb-2 font-mono", 
                    style: "border-bottom: 1px solid rgba(127, 168, 109, 0.3);",
                    "┌─ IMPORTED PLAYLISTS ─┐" 
                }
                
                match playlists_resource.read().as_ref() {
                    Some(Ok(all_playlists)) => {
                        // Filter playlists based on search term and status
                        let filtered_playlists = {
                            let search_lower = search_term.read().to_lowercase();
                            let status_filter = filter_status.read().clone();
                            
                            all_playlists.iter().filter(|p| {
                                // Apply search filter
                                let matches_search = search_lower.is_empty() || 
                                    p.name.to_lowercase().contains(&search_lower);
                                
                                // Apply status filter
                                let matches_status = match status_filter.as_str() {
                                    "not_enriched" => !p.has_enrichment,
                                    "partially_enriched" => p.has_enrichment && !p.fully_enriched,
                                    "fully_enriched" => p.fully_enriched,
                                    _ => true, // "all"
                                };
                                
                                matches_search && matches_status
                            }).cloned().collect::<Vec<ImportedPlaylist>>()
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
                                    "[ EMPTY ] - No imported playlists found. Visit the Import page first." 
                                }
                            }
                        } else {
                            rsx! {
                                div { class: "space-y-4",
                                    for playlist in filtered_playlists {
                                        EnrichmentPlaylistCard {
                                            playlist: playlist.clone(),
                                            enrichment_status: enrichment_status,
                                            enriching_playlist: enriching_playlist,
                                            enrichment_result: enrichment_result,
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(e)) => rsx! { 
                        p { class: "text-center py-8 font-mono", style: "color: #c1534d;", 
                            "[ ERROR ] - Database query failed: {e}" 
                        } 
                    },
                    None => rsx! { 
                        p { class: "text-center py-8 font-mono", style: "color: #648a54;", 
                            "[ LOADING ] - Scanning music library database..." 
                        } 
                    }
                }
            }
            
            // Results section
            if let Some(result) = enrichment_result.read().as_ref() {
                div { class: "card-glass p-6",
                    h2 { class: "text-xl font-semibold text-gradient mb-4 font-mono", 
                        "┌─ ENRICHMENT RESULTS ─┐" 
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

#[component]
fn EnrichmentPlaylistCard(
    playlist: ImportedPlaylist,
    enrichment_status: Signal<std::collections::HashMap<String, String>>,
    enriching_playlist: Signal<Option<String>>,
    enrichment_result: Signal<Option<String>>,
) -> Element {
    let is_enriching = enriching_playlist.read().as_ref() == Some(&playlist.id);
    let status_message = enrichment_status.read().get(&playlist.id).cloned();
    let mut song_count = use_signal(|| 3u32);
    let mut current_playlist = use_signal(|| playlist.clone());
    let mut animation_frame = use_signal(|| 0usize);
    
    // ASCII animation frames
    let animation_frames = [
        "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"
    ];
    
    // Animation effect for enriching state
    let playlist_id_for_animation = playlist.id.clone();
    use_effect({
        let animation_frame = animation_frame.clone();
        let enriching_playlist = enriching_playlist.clone();
        move || {
            if is_enriching {
                let playlist_id_clone = playlist_id_for_animation.clone();
                let mut animation_frame = animation_frame.clone();
                let enriching_playlist = enriching_playlist.clone();
                
                spawn(async move {
                    loop {
                        // Use web-compatible sleep
                        #[cfg(feature = "web")]
                        TimeoutFuture::new(100).await;
                        #[cfg(feature = "server")]
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        
                        if !enriching_playlist.read().as_ref().map_or(false, |id| id == &playlist_id_clone) {
                            break;
                        }
                        let current_frame = *animation_frame.read();
                        animation_frame.set((current_frame + 1) % animation_frames.len());
                    }
                });
            }
        }
    });

    rsx! {
        div { 
            class: "card-glass p-4 flex items-center justify-between",
            
            // Playlist info
            div { class: "flex items-center space-x-4 flex-1",
                // Playlist icon (since we don't have images from DB)
                div { class: "w-16 h-16 rounded-md flex items-center justify-center font-mono text-xl",
                    style: "background: rgba(100, 138, 84, 0.3); color: #7fa86d;",
                    "♪"
                }
                
                // Playlist details
                div { class: "flex-1",
                    h3 { class: "text-lg font-semibold text-gradient mb-1 font-mono",
                        "{current_playlist.read().name}"
                    }
                    if !current_playlist.read().description.is_empty() {
                        p { class: "text-sm mb-1 font-mono", style: "color: #648a54;",
                            "{current_playlist.read().description}"
                        }
                    }
                    p { class: "text-xs font-mono mb-1", style: "color: #7fa86d;",
                        "// {current_playlist.read().song_count} songs - {current_playlist.read().enrichment_status()}"
                    }
                    
                    // Progress bar for enrichment with smooth animation
                    if current_playlist.read().song_count > 0 {
                        div { class: "w-full bg-gray-700 rounded-full h-2 mt-2",
                            div { 
                                class: "h-2 rounded-full transition-all duration-500 ease-out",
                                style: format!(
                                    "width: {:.1}%; background: {};", 
                                    current_playlist.read().enrichment_percentage(),
                                    if current_playlist.read().fully_enriched { 
                                        "linear-gradient(90deg, #7fa86d, #9fc08e)" 
                                    } else if current_playlist.read().has_enrichment { 
                                        "linear-gradient(90deg, #c1534d, #7fa86d)" 
                                    } else { 
                                        "rgba(100, 138, 84, 0.3)" 
                                    }
                                ),
                            }
                        }
                        p { class: "text-xs font-mono mt-1", style: "color: #9fc08e;",
                            "{current_playlist.read().enriched_count}/{current_playlist.read().song_count} enriched ({current_playlist.read().enrichment_percentage():.1}%)"
                        }
                    }
                    
                    // Song count selector (only show when not enriching)
                    if !is_enriching && current_playlist.read().can_be_enriched() {
                        div { class: "mt-2 flex items-center gap-2",
                            label { class: "text-xs font-mono", style: "color: #7fa86d;", "Songs to enrich:" }
                            select {
                                class: "text-xs px-2 py-1 rounded font-mono",
                                style: "background: rgba(26, 34, 24, 0.8); color: #c1d4b6; border: 1px solid rgba(127, 168, 109, 0.3);",
                                value: "{song_count}",
                                onchange: move |evt| {
                                    if let Ok(count) = evt.value().parse::<u32>() {
                                        song_count.set(count);
                                    }
                                },
                                option { value: "1", "1 song" }
                                option { value: "3", "3 songs" }
                                option { value: "5", "5 songs" }
                                option { value: "10", "10 songs" }
                                option { value: "20", "20 songs" }
                            }
                        }
                    }
                }
            }
            
            // Status and action
            div { class: "flex items-center space-x-3",
                // Status message with animation
                if let Some(message) = status_message {
                    div { class: "text-sm font-mono flex items-center gap-1",
                        if is_enriching {
                            span { style: "color: #9fc08e;", "{animation_frames[*animation_frame.read()]}" }
                            span { style: "color: #9fc08e;", "[ PROCESSING ]" }
                        } else if message.contains("SUCCESS") || message.contains("✅") {
                            span { style: "color: #7fa86d;", "✅ [ SUCCESS ]" }
                        } else if message.contains("ERROR") || message.contains("❌") {
                            span { style: "color: #c1534d;", "❌ [ ERROR ]" }
                        } else {
                            span { style: "color: #9fc08e;", "[ PROCESSING ]" }
                        }
                    }
                }
                
                // Enrich button
                button {
                    disabled: is_enriching || !current_playlist.read().can_be_enriched(),
                    class: if is_enriching || !current_playlist.read().can_be_enriched() {
                        "btn-glass text-sm font-semibold opacity-50 cursor-not-allowed font-mono"
                    } else {
                        "btn-glass text-sm font-semibold sparkle font-mono"
                    },
                    onclick: move |_| {
                        let playlist_id = playlist.id.clone();
                        let count = *song_count.read();
                        enriching_playlist.set(Some(playlist_id.clone()));
                        enrichment_result.set(None); // Clear previous results
                        enrichment_status.write().insert(playlist_id.clone(), format!("🤖 Analyzing {} songs with LLM...", count));
                        
                        spawn(async move {
                            match enrich_playlist_from_database(playlist_id.clone(), count).await {
                                Ok(result) => {
                                    enrichment_status.write().insert(playlist_id.clone(), "✅ SUCCESS".to_string());
                                    enrichment_result.set(Some(result));
                                    
                                    // Update the playlist data in real-time
                                    if let Ok(updated_playlist) = get_playlist_enrichment_status(playlist_id.clone()).await {
                                        current_playlist.set(updated_playlist);
                                    }
                                }
                                Err(e) => {
                                    enrichment_status.write().insert(playlist_id, format!("❌ Error: {}", e));
                                }
                            }
                            enriching_playlist.set(None);
                        });
                    },
{
                    if is_enriching {
                        format!("[ ENRICHING {} SONGS... ]", *song_count.read())
                    } else if current_playlist.read().fully_enriched {
                        "[ FULLY ENRICHED ]".to_string()
                    } else if current_playlist.read().song_count == 0 {
                        "[ EMPTY ]".to_string()
                    } else {
                        format!("[ ENRICH {} SONGS ]", *song_count.read())
                    }
                }
                }
            }
        }
    }
}

#[component]
pub fn PlaylistGeneratorPage() -> Element {
    let node_types_resource: Resource<Result<Vec<NodeTypeInfo>, ServerFnError>> = use_server_future(|| async {
        get_available_node_types().await
    })?;
    
    let mut playlist_name = use_signal(|| "My Generated Playlist".to_string());
    let mut selected_filters = use_signal(Vec::<PlaylistFilter>::new);
    let mut max_songs = use_signal(|| 50u32);
    let mut shuffle_playlist = use_signal(|| true);
    let mut generation_result = use_signal(|| None::<String>);
    let mut is_generating = use_signal(|| false);
    
    // Selected node type and value for adding new filters
    let mut selected_node_type = use_signal(|| String::new());
    let mut selected_node_value = use_signal(|| String::new());
    let mut confidence_threshold = use_signal(|| 0.5f64);
    
    rsx! {
        div {
            class: "space-y-8 p-4 md:p-8",
            
            // Header
            div {
                class: "card-glass float p-8 text-center",
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╔═════════════════════════════╗"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ║    PLAYLIST GENERATOR      ║"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╚═════════════════════════════╝"
                }
                p { class: "text-lg font-mono", style: "color: #4f6d44;", 
                    "> Create semantic playlists from your music graph..." 
                }
                div { class: "mt-4 flex justify-center space-x-1 font-mono",
                    div { class: "animate-pulse", style: "color: #9fc08e;", "🎵" }
                    div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "→" }
                    div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "🤖" }
                    div { class: "animate-pulse", style: "color: #9fc08e;", "→" }
                    div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "📊" }
                }
            }
            
            // Configuration Section
            div {
                class: "card-glass p-6",
                h2 { class: "text-2xl font-semibold text-gradient mb-4 pb-2 font-mono", 
                     style: "border-bottom: 1px solid rgba(127, 168, 109, 0.3);", 
                     "┌─ PLAYLIST CONFIGURATION ─┐" 
                }
                
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    div {
                        label { class: "block text-sm font-medium mb-2 font-mono", style: "color: #4f6d44;", 
                               "Playlist Name:" }
                        input {
                            r#type: "text",
                            class: "input-glass w-full font-mono",
                            value: "{playlist_name}",
                            oninput: move |evt| playlist_name.set(evt.value()),
                            placeholder: "Enter playlist name..."
                        }
                    }
                    
                    div {
                        label { class: "block text-sm font-medium mb-2 font-mono", style: "color: #4f6d44;", 
                               "Max Songs:" }
                        select {
                            class: "input-glass w-full font-mono",
                            value: "{max_songs}",
                            onchange: move |evt| {
                                if let Ok(val) = evt.value().parse::<u32>() {
                                    max_songs.set(val);
                                }
                            },
                            option { value: "25", "25 songs" }
                            option { value: "50", selected: true, "50 songs" }
                            option { value: "100", "100 songs" }
                            option { value: "200", "200 songs" }
                        }
                    }
                }
                
                div { class: "mt-4 flex items-center space-x-2",
                    input {
                        r#type: "checkbox",
                        id: "shuffle-checkbox",
                        class: "w-4 h-4",
                        checked: *shuffle_playlist.read(),
                        onchange: move |evt| shuffle_playlist.set(evt.checked())
                    }
                    label { 
                        r#for: "shuffle-checkbox",
                        class: "text-sm font-medium font-mono", 
                        style: "color: #4f6d44;", 
                        "🔀 Shuffle playlist" 
                    }
                }
            }
            
            // Semantic Filters Section
            {
                match node_types_resource.read().as_ref() {
                    Some(Ok(node_types)) if !node_types.is_empty() => rsx! {
                        div {
                            class: "card-glass p-6",
                            h2 { class: "text-2xl font-semibold text-gradient mb-4 pb-2 font-mono", 
                                 style: "border-bottom: 1px solid rgba(127, 168, 109, 0.3);", 
                                 "┌─ SEMANTIC FILTERS ─┐" 
                            }
                            
                            div { class: "mb-6 p-4 border border-opacity-30", style: "border-color: #7fa86d;",
                                h3 { class: "text-lg font-semibold mb-3 font-mono", style: "color: #9fc08e;", 
                                     "Add Filter:" }
                                
                                div { class: "grid grid-cols-1 md:grid-cols-4 gap-3 items-end",
                                    div {
                                        label { class: "block text-sm font-medium mb-1 font-mono", style: "color: #4f6d44;", 
                                               "Category:" }
                                        select {
                                            class: "input-glass w-full font-mono text-sm",
                                            value: "{selected_node_type}",
                                            onchange: move |evt| {
                                                selected_node_type.set(evt.value());
                                                selected_node_value.set(String::new());
                                            },
                                            option { value: "", "Select category..." }
                                            for node_type in node_types {
                                                option { value: "{node_type.node_type}", "{node_type.node_type}" }
                                            }
                                        }
                                    }
                                    
                                    div {
                                        label { class: "block text-sm font-medium mb-1 font-mono", style: "color: #4f6d44;", 
                                               "Value:" }
                                        select {
                                            class: "input-glass w-full font-mono text-sm",
                                            value: "{selected_node_value}",
                                            disabled: selected_node_type.read().is_empty(),
                                            onchange: move |evt| selected_node_value.set(evt.value()),
                                            option { value: "", "Select value..." }
                                            
                                            {
                                                if let Some(current_node_type) = node_types.iter().find(|nt| nt.node_type == *selected_node_type.read()) {
                                                    rsx! {
                                                        for value in &current_node_type.available_values {
                                                            option { 
                                                                value: "{value.value}", 
                                                                "{value.value} ({value.song_count} songs)" 
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    rsx! {}
                                                }
                                            }
                                        }
                                    }
                                    
                                    {
                                        if ["Genre", "Culture", "UserTag"].contains(&selected_node_type.read().as_str()) {
                                            rsx! {
                                                div {
                                                    label { class: "block text-sm font-medium mb-1 font-mono", style: "color: #4f6d44;", 
                                                           "Min Confidence:" }
                                                    input {
                                                        r#type: "number",
                                                        class: "input-glass w-full font-mono text-sm",
                                                        min: "0",
                                                        max: "1",
                                                        step: "0.1",
                                                        value: "{confidence_threshold}",
                                                        oninput: move |evt| {
                                                            if let Ok(val) = evt.value().parse::<f64>() {
                                                                confidence_threshold.set(val);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            rsx! { div {} }
                                        }
                                    }
                                    
                                    div {
                                        button {
                                            class: "btn-glass text-sm font-semibold font-mono",
                                            disabled: selected_node_type.read().is_empty() || selected_node_value.read().is_empty(),
                                            onclick: move |_| {
                                                let node_type = selected_node_type.read().clone();
                                                let value = selected_node_value.read().clone();
                                                
                                                if !node_type.is_empty() && !value.is_empty() {
                                                    let mut filters = selected_filters.read().clone();
                                                    
                                                    if !filters.iter().any(|f| f.node_type == node_type && f.value == value) {
                                                        let min_confidence = if ["Genre", "Culture", "UserTag"].contains(&node_type.as_str()) {
                                                            Some(*confidence_threshold.read())
                                                        } else {
                                                            None
                                                        };
                                                        
                                                        filters.push(PlaylistFilter {
                                                            node_type: node_type.clone(),
                                                            value: value.clone(),
                                                            min_confidence,
                                                        });
                                                        
                                                        selected_filters.set(filters);
                                                        selected_node_type.set(String::new());
                                                        selected_node_value.set(String::new());
                                                    }
                                                }
                                            },
                                            "[ ADD FILTER ]"
                                        }
                                    }
                                }
                            }
                            
                            {
                                if !selected_filters.read().is_empty() {
                                    rsx! {
                                        div { class: "mb-6",
                                            h3 { class: "text-lg font-semibold mb-3 font-mono", style: "color: #9fc08e;", 
                                                 "Active Filters ({selected_filters.read().len()}):" }
                                            
                                            div { class: "flex flex-wrap gap-2",
                                                for (index, filter) in selected_filters.read().iter().enumerate() {
                                                    div { 
                                                        class: "bg-opacity-20 px-3 py-1 rounded flex items-center space-x-2 font-mono text-sm",
                                                        style: "background-color: #7fa86d; color: #dde7d5;",
                                                        span { 
                                                            "{filter.node_type}: {filter.value}"
                                                            {
                                                                if let Some(conf) = filter.min_confidence {
                                                                    format!(" (≥{})", conf)
                                                                } else {
                                                                    String::new()
                                                                }
                                                            }
                                                        }
                                                        button {
                                                            class: "ml-2 text-red-400 hover:text-red-300 text-xs",
                                                            onclick: move |_| {
                                                                let mut filters = selected_filters.read().clone();
                                                                filters.remove(index);
                                                                selected_filters.set(filters);
                                                            },
                                                            "✕"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    rsx! {}
                                }
                            }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        div { class: "card-glass p-6",
                            p { class: "text-center py-8 font-mono", style: "color: #648a54;", 
                               "[ NO SEMANTIC DATA ] - No enriched songs found." }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "card-glass p-6",
                            p { class: "text-center py-8 font-mono", style: "color: #c1534d;", 
                               "[ ERROR ] - Failed to load node types: {e}" }
                        }
                    },
                    None => rsx! {
                        div { class: "card-glass p-6",
                            p { class: "text-center py-8 font-mono", style: "color: #648a54;", 
                               "[ LOADING ] - Scanning music graph..." }
                        }
                    }
                }
            }
            
            // Generate Button
            div {
                class: "card-glass p-6 text-center",
                
                button {
                    class: if *is_generating.read() || selected_filters.read().is_empty() {
                        "btn-glass text-lg font-semibold opacity-50 cursor-not-allowed font-mono"
                    } else {
                        "btn-glass text-lg font-semibold sparkle font-mono"
                    },
                    disabled: *is_generating.read() || selected_filters.read().is_empty(),
                    onclick: move |_| {
                        let request = PlaylistGenerationRequest {
                            name: playlist_name.read().clone(),
                            description: Some("Generated by Betterd Spotify semantic engine".to_string()),
                            filters: selected_filters.read().clone(),
                            max_songs: *max_songs.read(),
                            shuffle: *shuffle_playlist.read(),
                        };
                        
                        is_generating.set(true);
                        generation_result.set(None);
                        
                        spawn(async move {
                            match generate_semantic_playlist(request).await {
                                Ok(result) => {
                                    let success_msg = format!(
                                        "🎵 SUCCESS! Created playlist: '{}'\n📊 {} songs added\n🔗 {}\n\n📋 Applied filters:\n{}",
                                        result.name,
                                        result.song_count,
                                        result.external_url,
                                        result.applied_filters.join("\n")
                                    );
                                    generation_result.set(Some(success_msg));
                                }
                                Err(e) => {
                                    generation_result.set(Some(format!("❌ Error: {}", e)));
                                }
                            }
                            is_generating.set(false);
                        });
                    },
                    {
                        if *is_generating.read() {
                            "[ GENERATING PLAYLIST... ]".to_string()
                        } else if selected_filters.read().is_empty() {
                            "[ ADD FILTERS TO GENERATE ]".to_string()
                        } else {
                            format!("[ GENERATE PLAYLIST ({} filters) ]", selected_filters.read().len())
                        }
                    }
                }
                
                {
                    if let Some(result) = generation_result.read().as_ref() {
                        rsx! {
                            div { 
                                class: "mt-6 p-4 font-mono text-sm whitespace-pre-line text-left",
                                style: "background-color: rgba(20, 30, 17, 0.8); border: 1px solid rgba(127, 168, 109, 0.3); border-radius: 8px;",
                                "{result}"
                            }
                        }
                    } else {
                        rsx! {}
                    }
                }
            }
        }
    }
}

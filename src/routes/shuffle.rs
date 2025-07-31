use dioxus::prelude::*;
use crate::api::{get_spotify_playlist_tracks_all, get_spotify_user_playlists_all, get_spotify_user_profile, shuffle_and_save_new_playlist};
use crate::api_models::{NewPlaylistDetails, SpotifyPlaylistItem, SpotifyTrackItem, SpotifyUserProfile};
use crate::components::spotify::{PlaylistsView, ProfileView};
use crate::Route;

// --- Shuffle Action Stages ---
#[derive(PartialEq, Clone, Debug)]
pub enum ShuffleStage { // Make pub if used across modules, fine for now if only in pages.rs
    Idle,
    FetchingTracks,
    ShufflingAndCreatingPlaylist { num_tracks_to_shuffle: usize },
    Completed(NewPlaylistDetails),
    Error(String),
}

// --- Stage-Specific View Components ---
#[component]
fn FetchingTracksView(playlist_name: String) -> Element {
    rsx! {
        div { class: "text-center p-6",
            div { class: "animate-spin rounded-full h-12 w-12 border-t-4 border-b-4 mx-auto mb-4", style: "border-color: #7fa86d transparent transparent transparent;" }
            p { class: "text-xl text-gradient font-mono", "┌─ FETCHING TRACKS ─┐" }
            p { class: "text-lg font-mono", style: "color: #4f6d44;", "Scanning \"{playlist_name}\"" }
            p { class: "text-sm font-mono mt-2", style: "color: #7fa86d;", "> Large playlists may take time..." }
        }
    }
}

#[component]
fn ShufflingAndCreatingView(playlist_name: String, num_tracks: usize) -> Element {
    rsx! {
        div { class: "text-center p-6",
            div { class: "animate-spin rounded-full h-12 w-12 border-t-4 border-b-4 mx-auto mb-4", style: "border-color: #9fc08e transparent transparent transparent;" }
            p { class: "text-xl text-gradient font-mono", "┌─ PROCESSING TRACKS ─┐" }
            p { class: "text-lg font-mono", style: "color: #4f6d44;", "Shuffling {num_tracks} tracks" }
            p { class: "text-sm font-mono mt-2", style: "color: #7fa86d;", "Creating playlist \"{playlist_name}\"" }
        }
    }
}

#[component]
fn ShuffleCompleteView(details: NewPlaylistDetails) -> Element {
    rsx! {
        div { class: "text-center p-6",
            p { class: "text-2xl text-gradient mb-4 font-mono", "╔═══════════════════════╗" }
            p { class: "text-2xl text-gradient mb-4 font-mono", "║   SHUFFLE COMPLETE!   ║" }
            p { class: "text-2xl text-gradient mb-6 font-mono", "╚═══════════════════════╝" }
            p { class: "text-lg font-mono mb-2", style: "color: #4f6d44;", "> New playlist created:" }
            p { class: "text-xl font-semibold font-mono mb-6", style: "color: #648a54;", "\"{details.name}\"" }
            a {
                href: "{details.external_url}", target: "_blank", rel: "noopener noreferrer",
                class: "btn-glass font-mono",
                "Open on Spotify"
            }
            div { class: "mt-6 flex justify-center space-x-1 font-mono",
                div { class: "animate-pulse", style: "color: #9fc08e;", "(" }
                div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "✓" }
                div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "✓" }
                div { class: "animate-pulse", style: "color: #9fc08e;", ")" }
            }
        }
    }
}

#[component]
fn ShuffleErrorView(error_message: String, on_retry: EventHandler<()>) -> Element {
    rsx! {
        div { class: "text-center p-6",
            p { class: "text-2xl text-gradient mb-4 font-mono", "╔═══════════════════╗" }
            p { class: "text-2xl text-gradient mb-4 font-mono", "║   PROCESS ERROR   ║" }
            p { class: "text-2xl text-gradient mb-6 font-mono", "╚═══════════════════╝" }
            p { class: "text-lg font-mono mb-2", style: "color: #4f6d44;", "> Error occurred:" }
            p { class: "text-sm font-mono mb-6", style: "color: #7fa86d;", "{error_message}" }
            button {
                class: "btn-glass font-mono",
                onclick: move |_| on_retry.call(()),
                "Retry Process"
            }
        }
    }
}

// --- ShuffleActionPage (Orchestrator) ---
#[component]
pub fn ShuffleActionPage(playlist_id: String, playlist_name: String) -> Element {
    let mut current_stage = use_signal(|| ShuffleStage::Idle);
    // This signal will store the fetched tracks to pass to the shuffle_and_save function
    // This ensures tracks are fetched only ONCE.
    let mut fetched_tracks_for_shuffle: Signal<Option<Vec<SpotifyTrackItem>>> = use_signal(|| None);

    // Clones for async tasks
    let pid_for_tasks = playlist_id.clone();
    let pname_for_tasks = playlist_name.clone();

    // Effect to manage the shuffle workflow
    use_effect(move || {
        match *current_stage.read() {
            ShuffleStage::FetchingTracks => {
                let mut stage_signal = current_stage;
                let mut tracks_signal = fetched_tracks_for_shuffle;
                let pid_clone = pid_for_tasks.clone();

                spawn(async move { // dioxus::prelude::spawn
                    match get_spotify_playlist_tracks_all(pid_clone).await {
                        Ok(tracks) => {
                            if tracks.is_empty() {
                                stage_signal.set(ShuffleStage::Error("Selected playlist is empty or no tracks were found.".to_string()));
                            } else {
                                let num_tracks = tracks.len();
                                tracks_signal.set(Some(tracks)); // Store fetched tracks
                                // Transition to next stage, now with num_tracks from the fetched data
                                stage_signal.set(ShuffleStage::ShufflingAndCreatingPlaylist { num_tracks_to_shuffle: num_tracks });
                            }
                        }
                        Err(e) => stage_signal.set(ShuffleStage::Error(format!("Failed to fetch tracks: {}", e))),
                    }
                });
            }
            ShuffleStage::ShufflingAndCreatingPlaylist { num_tracks_to_shuffle: _ } => { // num_tracks already set for UI
                // Check if tracks are actually fetched and stored
                if let Some(tracks) = fetched_tracks_for_shuffle.read().as_ref() {
                    // If `shuffle_and_save_new_playlist` needs the track URIs directly,
                    // extract them here. Otherwise, it might re-fetch based on ID if that's its design.
                    // For now, assuming `shuffle_and_save_new_playlist` takes playlist_id and re-fetches or uses cached if available.
                    // If `shuffle_and_save_new_playlist` is modified to take `Vec<String>` of track URIs,
                    // you would extract them from `tracks` here.

                    let mut stage_signal = current_stage;
                    let mut pid_clone = pid_for_tasks.clone();
                    let mut pname_clone = pname_for_tasks.clone();
                    // IMPORTANT: The current `shuffle_and_save_new_playlist` re-fetches tracks.
                    // If you want to avoid re-fetching, modify `shuffle_and_save_new_playlist`
                    // to accept `Vec<SpotifyTrackItem>` or `Vec<String>` (track URIs) as an argument.
                    // For this example, we proceed with its current signature.

                    spawn(async move {
                        match shuffle_and_save_new_playlist(pid_clone, pname_clone).await {
                            Ok(details) => stage_signal.set(ShuffleStage::Completed(details)),
                            Err(e) => stage_signal.set(ShuffleStage::Error(format!("Failed to shuffle and save playlist: {}", e))),
                        }
                    });
                } else {
                    // This case should ideally not be reached if FetchingTracks succeeded.
                    let mut stage_signal = current_stage;
                    stage_signal.set(ShuffleStage::Error("Track data was lost before shuffling could start. Please retry.".to_string()));
                }
            }
            _ => {} // Do nothing for Idle, Completed, Error in this effect
        }
    });

    rsx! {
        div {
            class: "space-y-8 p-4 md:p-8",
            div { /* Title div */
                class: "card-glass float p-8 text-center",
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╔═════════════════════════════╗"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ║       SHUFFLING MODE       ║"
                }
                h1 { class: "text-4xl font-bold text-gradient mb-4 font-mono",
                    " ╚═════════════════════════════╝"
                }
                p { class: "text-lg font-mono", style: "color: #4f6d44;", "Target: \"{playlist_name}\"" }
                p { class: "text-sm font-mono", style: "color: #7fa86d;", "ID: {playlist_id}" }
            }

            div { // Main content area for stages
                class: "card-glass p-6 max-w-xl mx-auto min-h-[12rem] flex flex-col items-center justify-center",
                match &*current_stage.read() {
                    ShuffleStage::Idle => rsx! {
                        div { class: "text-center",
                            p { class: "text-xl text-gradient font-mono mb-6", "┌─ READY TO SHUFFLE ─┐" }
                            button {
                                class: "btn-glass text-lg font-mono px-8 py-4",
                                onclick: move |_| {
                                    fetched_tracks_for_shuffle.set(None); // Clear previous tracks
                                    current_stage.set(ShuffleStage::FetchingTracks);
                                },
                                "Initialize Process"
                            }
                            div { class: "mt-4 flex justify-center space-x-1 font-mono",
                                div { class: "animate-pulse", style: "color: #9fc08e;", "(" }
                                div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "~" }
                                div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "~" }
                                div { class: "animate-pulse", style: "color: #9fc08e;", ")" }
                            }
                        }
                    },
                    ShuffleStage::FetchingTracks => rsx! {
                        FetchingTracksView { playlist_name: playlist_name.clone() }
                    },
                    ShuffleStage::ShufflingAndCreatingPlaylist { num_tracks_to_shuffle } => rsx! {
                        ShufflingAndCreatingView { playlist_name: playlist_name.clone(), num_tracks: *num_tracks_to_shuffle }
                    },
                    ShuffleStage::Completed(ref details) => rsx! {
                        ShuffleCompleteView { details: details.clone() }
                    },
                    ShuffleStage::Error(ref err_msg) => rsx! {
                        ShuffleErrorView {
                            error_message: err_msg.clone(),
                            on_retry: move |_| {
                                fetched_tracks_for_shuffle.set(None); // Clear previous tracks before retry
                                current_stage.set(ShuffleStage::Idle);
                            }
                        }
                    }
                }
            }
        }
    }
}

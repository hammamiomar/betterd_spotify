use crate::api_models::{SpotifyPlaylistItem, SpotifyUserProfile};
use dioxus::prelude::*;

#[component]
pub fn ProfileView(profile: SpotifyUserProfile) -> Element {
    rsx! {
        div {
            id: "user-profile-details",
            class: "flex items-center space-x-4", // Layout for image and text
            if let Some(images) = &profile.images {
                if let Some(image) = images.first() {
                    img {
                        src: "{image.url}",
                        class: "w-16 h-16 rounded-xl object-cover",
                        style: "border: 2px solid rgba(127, 168, 109, 0.5);",
                        alt: "User profile picture"
                    }
                } else {
                    div { 
                        class: "w-16 h-16 rounded-xl flex items-center justify-center font-mono text-sm",
                        style: "background: rgba(100, 138, 84, 0.3); color: #7fa86d; border: 2px solid rgba(127, 168, 109, 0.5);",
                        "[USR]"
                    }
                }
            } else {
                div { 
                    class: "w-16 h-16 rounded-xl flex items-center justify-center font-mono text-sm",
                    style: "background: rgba(100, 138, 84, 0.3); color: #7fa86d; border: 2px solid rgba(127, 168, 109, 0.5);",
                    "[USR]"
                }
            }
            div {
                p { class: "text-xl text-gradient font-mono", ">>> Welcome, {profile.display_name}" }
                p { class: "text-sm font-mono", style: "color: #648a54;", "// USER_ID: {profile.id}" }
            }
        }
    }
}


#[component]
pub fn PlaylistsView(playlists: Vec<SpotifyPlaylistItem>, selected_playlist: Signal<Option<SpotifyPlaylistItem>>) -> Element {
    let currently_selected_id_opt: Option<String> = selected_playlist
        .read()
        .as_ref()
        .map(|p| p.id.clone());

    rsx! {
        div {
            id: "playlist-list-details",
                ul {
                    class: "space-y-3 max-h-96 overflow-y-auto scrollbar-sage",
                    if playlists.is_empty(){
                        li{class: "text-center p-6 font-mono", style: "color: #648a54;", "[ EMPTY ] - No audio collections detected"}
                    }else{
                        {playlists.iter().map(|playlist_item| {
                            let is_selected = match &currently_selected_id_opt {
                                Some(selected_id_str) => &playlist_item.id == selected_id_str,
                                None => false,
                            };

                            let item_classes = if is_selected {
                                "card-glass p-4 flex items-center justify-between cursor-pointer"
                            } else {
                                "glass p-4 rounded-2xl flex items-center justify-between hover:scale-[1.01] transition-all duration-300 cursor-pointer"
                            };

                            let item_style = if is_selected {
                                "background: rgba(127, 168, 109, 0.3); border-color: rgba(127, 168, 109, 0.5);"
                            } else {
                                ""
                            };

                            let item_for_click_closure = playlist_item.clone();
                            let mut signal_for_click_closure = selected_playlist;

                            rsx! {
                                li {
                                    key: "{playlist_item.id}",
                                    class: "{item_classes}",
                                    style: "{item_style}",
                                    onclick: move |_| {
                                        //log::info!("clicked playlistL{}",item_for_click_closure.id);
                                        signal_for_click_closure.set(Some(item_for_click_closure.clone()));
                                    },
                                    div { // For image and name/description
                                        class: "flex items-center space-x-4",
                                        if let Some(images) = &playlist_item.images {
                                            if let Some(image) = images.first() {
                                                img {
                                                    src: "{image.url}",
                                                    alt: "{playlist_item.name} cover",
                                                    class: "playlist-image w-12 h-12 object-cover rounded-xl"
                                                }
                                            } else {
                                                div { 
                                                    class: "w-12 h-12 rounded-xl flex items-center justify-center text-xs font-mono",
                                                    style: "background: rgba(100, 138, 84, 0.3); color: #7fa86d;",
                                                    "♪"
                                                }
                                            }
                                        } else {
                                            div { 
                                                class: "w-12 h-12 rounded-xl flex items-center justify-center text-xs font-mono",
                                                style: "background: rgba(100, 138, 84, 0.3); color: #7fa86d;",
                                                "♪"
                                            }
                                        }
                                        div {
                                            p { class: "font-semibold text-gradient font-mono", "{playlist_item.name}" }
                                            if let Some(desc) = &playlist_item.description {
                                                if !desc.is_empty() { 
                                                    p { class: "text-xs font-mono truncate w-64", style: "color: #648a54;", "{desc}" } 
                                                }
                                            }
                                        }
                                    }
                                    if is_selected {
                                        div { class: "font-mono text-sm", style: "color: #7fa86d;", "[ SELECTED ]" }
                                    }
                                }
                            }
                        })}
                    } 
                }
            }
        }
    }


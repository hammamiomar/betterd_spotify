#![allow(non_snake_case)]
use dioxus::prelude::*;

mod components;
mod routes;

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
mod auth;
#[cfg(feature="server")]
mod db;



pub mod api;
pub mod api_models;
pub mod db_operations;

use crate::components::layout::*;
use crate::routes::pages::*;
use crate::routes::shuffle::*;

static CSS: Asset = asset!("/assets/tailwind.css");

// The entry point for the server
#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    if let Err(e) = server::start_server().await{
        eprintln!("Server failed to start: {}", e);
        std::process::exit(1);
    }
}
// For any other platform, we just launch the app
#[cfg(not(feature = "server"))]
fn main() {
     wasm_logger::init(wasm_logger::Config::default());
    // If using console_log and console_error_panic_hook:
    // std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    // console_log::init_with_level(log::Level::Info).expect("Failed to init console_log");

    log::info!("Dioxus client application started"); 
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    use_context_provider(|| Signal::new(false));
    
    rsx! {
        document::Stylesheet { href:CSS}
        div{ class:"min-h-screen flex flex-col", style: "background: linear-gradient(135deg, #1a2218, #2c3e28); color: #dde7d5;",
            Router::<Route>{ }
        }
    }
}

//Router
#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[layout(NavBar)]
    #[route("/")]
    Home {},
    #[route("/login")]
    LoginPage {},
    #[route("/shuffle")]
    ShufflePage{},
    #[route("/shuffle/:playlist_id/:playlist_name")]
    ShuffleActionPage{playlist_id:String, playlist_name: String},
    #[route("/test-audio")]
    AudioFeaturesTestPage{},
    #[route("/import-data")]
    ImportDataPage{},
    #[route("/callback")]
    CallBack{},
}
#[component]
fn CallBack() -> Element{
    todo!()
}


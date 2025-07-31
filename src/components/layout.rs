use dioxus::{document::eval, prelude::*};
use crate::{api::{check_auth, logout}, Route};

#[component]
pub fn NavBar() -> Element {
    let is_authenticated = use_server_future( || async {
        check_auth().await})?;

    rsx! {
        header {
            class: "nav-glass p-4",
            nav {
                class: "container mx-auto flex justify-between items-center",

                div{class: "text-xl font-bold text-gradient hover:scale-105 transition-transform font-mono",
                     Link {to:Route::Home{}, "[ BETTER_SPOTIFY ]"}
                    }
                ul {class: "flex space-x-6",

                    li { Link { to: Route::Home {}, class: "font-medium transition-colors font-mono", style: "color: #4f6d44;", "[ HOME ]" } }

                    match is_authenticated.read().as_ref(){
                        None => rsx!{}, // Still loading, show nothing
                        Some(Err(_e)) => rsx!{li {Link {to:Route::LoginPage {  }, class: "btn-glass text-sm font-mono", "[ LOGIN ]"}} },
                        Some(Ok(false)) => rsx!{li {Link {to:Route::LoginPage {  }, class: "btn-glass text-sm font-mono", "[ LOGIN ]"}} },
                        Some(Ok(true)) => rsx!{
                            li {Link {to:Route::ShufflePage{  }, class: "font-medium transition-colors font-mono", style: "color: #4f6d44;", "[ SHUFFLE ]"}}
                            li {Link {to:Route::ImportDataPage{  }, class: "font-medium transition-colors font-mono", style: "color: #4f6d44;", "[ IMPORT ]"}}
                            li {Link {to:Route::AudioFeaturesTestPage{  }, class: "font-medium transition-colors font-mono", style: "color: #4f6d44;", "[ ANALYZE ]"}}

                            li {
                                button {
                                    class: "btn-glass text-sm font-mono",
                                    style: "background: rgba(196, 84, 77, 0.2); border-color: rgba(196, 84, 77, 0.3); color: #c1534d;",
                                    onclick: move |_| async move {
                                        let _ = logout().await;
                                        let _ = eval(r#"window.location.href = "/auth/logout";"#);
                                    },
                                    "[ LOGOUT ]"
                                }
                        
                            }
                        }
                    }
                }
            }
        }
        main { class: "flex-grow container mx-auto p-6 scrollbar-sage",
            SuspenseBoundary { 
                fallback: |_| rsx!{LoadingSpinner{}},
                Outlet::<Route>{}
            }
            
        }
        Footer {  }
    }
}

#[component]
pub fn Footer() -> Element {
    rsx! {
        footer {
            class: "glass glass-dark p-4 text-center mt-auto border-t border-sage-200/20 dark:border-sage-700/30",
            p {
                class: "font-mono",
                style: "color: #648a54;",
                "© 2025 BETTER_SPOTIFY - Built by "
                a { 
                    href: "https://hammamiomar.xyz", 
                    target: "_blank", 
                    rel: "noopener noreferrer", 
                    class: "text-gradient hover:scale-105 transition-transform inline-block font-mono", 
                    "[ O.HAMMAMI ]" 
                }
            }
        }
    }

    
}

#[component]
fn LoadingSpinner() -> Element {
    rsx! {
        div {
            class: "flex flex-col items-center justify-center p-8 text-center",
            div {
                class: "card-glass float p-8 max-w-sm mx-auto",
                div {
                    class: "animate-spin rounded-full h-16 w-16 mx-auto",
                    style: "border: 4px solid #dde7d5; border-top: 4px solid #7fa86d;",
                }
                p {
                    class: "mt-6 text-xl text-gradient font-mono",
                    "[ LOADING... ]"
                }
                p {
                    class: "mt-2 text-sm font-mono",
                    style: "color: #648a54;",
                    "> Processing audio data..."
                }
                div {
                    class: "mt-4 flex justify-center space-x-1 font-mono",
                    div { class: "animate-pulse", style: "color: #9fc08e;", "[" }
                    div { class: "animate-pulse animation-delay-75", style: "color: #7fa86d;", "■" }
                    div { class: "animate-pulse animation-delay-150", style: "color: #648a54;", "]" }
                }
            }
        }
    }
}
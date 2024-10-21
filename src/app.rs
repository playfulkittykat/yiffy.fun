/*
 * Yiffy.Fun
 *
 * Copyright (C) 2022,2024 Playful KittyKat
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
use base64::engine::general_purpose::STANDARD;
use base64::prelude::*;

use bevy_pkv::PkvStore;
use serde::{Deserialize, Serialize};

use crate::tag;
use crate::yiff::Yiff;

use dioxus::prelude::*;
use keyboard_types::Key;

use rs621::post::{Post, PostFileExtension, Query};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::lock::Mutex;

use url::Url;

const BASE_URL: &str = "https://e621.net";

lazy_static::lazy_static! {
    static ref LOGO_E621: String = format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(include_bytes!("assets/e621.svg")),
    );
}

#[derive(Debug, Clone)]
struct Search {
    search: Arc<Mutex<crate::yiff::Search>>,
    yiff: Yiff,
}

impl Search {
    pub fn new<Q: Into<Query>>(yiff: Yiff, query: Q) -> Self {
        let query = query.into();
        let search = yiff.search(query);

        Search {
            search: Arc::new(Mutex::new(search)),
            yiff,
        }
    }
}

fn store() -> PkvStore {
    PkvStore::new_with_qualifier("fun", "yiffy", env!("CARGO_PKG_NAME"))
}

#[derive(Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
enum Hand {
    #[default]
    Left,
    Right,
}

impl Hand {
    async fn load() -> Self {
        // TODO: Find a spawn_blocking replacement.
        store().get("hand").unwrap_or_default()
    }

    async fn save(self) {
        // TODO: Find a spawn_blocking replacement.
        store().set("hand", &self).unwrap()
    }
}

#[derive(Props, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
struct Credentials {
    username: String,
    api_key: String,
    active: bool,
}

impl Credentials {
    async fn load() -> Self {
        // TODO: Find a spawn_blocking replacement.
        store().get("credentials").unwrap_or_default()
    }

    async fn save(self) {
        // TODO: Find a spawn_blocking replacement.
        store().set("credentials", &self).unwrap()
    }
}

#[derive(Debug, Default)]
struct ActiveQuery {
    terms: Vec<String>,
    active: bool,
}

pub(crate) fn app() -> Element {
    // Prevent scrolling with keyboard:
    use_future(|| {
        eval(
            r##"window.addEventListener(
        'keydown',
        (e) => {
            if (e.target.matches("#viewport-wrapper, #viewport-wrapper *")) {
                e.preventDefault()
            }
        }
    )"##,
        )
        .join()
    });

    let hand = use_resource(Hand::load);
    let hand = match *hand.read_unchecked() {
        Some(a) => a,
        None => return rsx! { "Loading hand..." },
    };

    let credentials = use_resource(Credentials::load);
    let credentials = match &*credentials.read_unchecked() {
        Some(a) => a.clone(),
        None => return rsx! { "Loading credentials..." },
    };

    let mut query = use_signal(ActiveQuery::default);
    let mut credentials_signal = use_signal(|| credentials.clone());
    let mut hand_signal = use_signal(|| hand);

    if !credentials_signal.read().active {
        return rsx! {
            crate::app::login { credentials: credentials_signal }
            crate::app::notice {}
        };
    }

    let yiff = use_signal(|| {
        let creds = credentials_signal.read();
        Yiff::new(BASE_URL, "pkk@tabby.rocks", &creds.username, &creds.api_key)
    });

    let options_style = include_str!("options.css");
    let set_hand = move |e: Event<FormData>| {
        let mut signal = hand_signal.write();
        let hand = match &*e.value() {
            "left" => Hand::Left,
            "right" => Hand::Right,
            _ => unreachable!(),
        };

        if hand == *signal {
            return;
        }

        *signal = hand;
        spawn_forever(hand.save());
    };

    let entries = tag::Entries::new();
    match &*query.read_unchecked() {
        q if !q.active => rsx! {
            tag::List {
                entries,
                yiff,
                onsubmit: move |terms| {
                    let mut query = query.write();
                    query.terms = terms;
                    query.active = true;
                }
            }
            style { "{options_style}" }
            form { class: "options", action: "#", prevent_default: "onsubmit",
                fieldset {
                    legend { "In which hand is your phone?" }
                    label {
                        input {
                            r#type: "radio",
                            name: "hand",
                            value: "left",
                            oninput: set_hand,
                            checked: *hand_signal.read() == Hand::Left
                        }
                        "Left"
                    }
                    label {
                        input {
                            r#type: "radio",
                            name: "hand",
                            value: "right",
                            oninput: set_hand,
                            checked: *hand_signal.read() == Hand::Right
                        }
                        "Right"
                    }
                }
                fieldset {
                    legend { "Want to disconnect your account?" }
                    button {
                        class: "log-out",
                        tabindex: "-1",
                        onclick: move |_| {
                            *credentials_signal.write() = Default::default();
                            spawn_forever(Credentials::default().save());
                        },
                        "Log Out"
                    }
                }
            }
            crate::app::notice {}
        },
        _ => rsx! {
            crate::app::viewer { yiff, credentials: credentials_signal, query, hand: hand_signal }
        },
    }
}

#[component]
fn ExternalLink(href: Url, children: Element) -> Element {
    let href_clone = href.clone();
    rsx! {
        a {
            prevent_default: "onclick",
            onclick: move |_| {
                webbrowser::open(href_clone.as_str()).ok();
            },
            href: "{href}",
            target: "_blank",
            {children}
        }
    }
}

fn notice() -> Element {
    let year = &env!("VERGEN_GIT_COMMIT_TIMESTAMP")[..4];
    let source = Url::parse(concat!(env!("CARGO_PKG_REPOSITORY"), "/"))
        .unwrap()
        .join("commit/")
        .unwrap()
        .join(env!("VERGEN_GIT_SHA"))
        .unwrap();

    let license = env!("CARGO_PKG_LICENSE");
    let notice_style = include_str!("notice.css");

    rsx! {
        footer { class: "copyright",

            style { "{notice_style}" }
            "Copyright {year}. "
            "Available under the terms of {license}. "
            crate::app::ExternalLink { href: source, "Source code" }
            "."
        }
    }
}

#[component]
fn login(credentials: Signal<Credentials>) -> Element {
    let login_style = include_str!("login.css");
    rsx! {
        style { "{login_style}" }

        div { style: "display: inline-block; width: min-content;",

            form {
                prevent_default: "onsubmit",
                onsubmit: move |_| {
                    let mut creds = credentials.write();
                    creds.active = true;
                    spawn_forever(creds.clone().save());
                },

                label {
                    "Username:"
                    input {
                        "type": "text",
                        value: "{credentials.read().username}",
                        oninput: move |evt| {
                            credentials.write().username = evt.value().clone();
                        }
                    }
                }

                label {
                    "API Key:"
                    input {
                        "type": "password",
                        value: "{credentials.read().api_key}",
                        minlength: "24",
                        oninput: move |evt| {
                            credentials.write().api_key = evt.value().clone();
                        }
                    }
                }

                button { "type": "submit", "Log In" }
            }

            div { class: "help",

                "Your API Key is "
                strong { "not" }
                " your password. You can find your API Key under "
                strong { "Account > Manage API Access" }
                " once logged into e621."
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Preload {
    href: String,
    kind: String,
}

#[component]
fn viewer(
    yiff: ReadOnlySignal<Yiff>,
    credentials: Signal<Credentials>,
    query: Signal<ActiveQuery>,
    hand: Signal<Hand>,
) -> Element {
    let search = use_resource(move || async move {
        let creds = credentials.read();
        let query_ref = query.read();
        // let yiff = Yiff::new("https://e926.net", "pkk@tabby.rocks");

        let faved = format!("-favoritedby:{}", creds.username);

        // TODO: Verify that this is how I should be splitting query terms.
        let mut query_terms: Vec<_> = query_ref.terms.iter().map(String::as_str).collect();

        query_terms.push("order:random");
        query_terms.push("score:>=0");
        query_terms.push("-voted:anything");
        query_terms.push("-type:swf");
        query_terms.push(&faved);

        let yiff = yiff.read().clone();
        Search::new(yiff, query_terms.as_slice())
    });

    let search = match &*search.read_unchecked() {
        Some(s) => s.clone(),
        None => return rsx! { "Searching..." },
    };

    let mut disabled = use_signal(|| false);
    let mut current = use_signal(|| Option::<Arc<Post>>::None);
    let mut preload = use_signal(Preload::default);

    let search_clone = search.search.clone();
    let mut advance = use_future(move || {
        let search_clone = search_clone.clone();
        async move {
            let mut guard = search_clone.lock().await;
            let reply = guard.next().await;

            let post = match reply {
                Err(_) => None,   // TODO
                Ok(None) => None, // TODO
                Ok(Some(p)) => Some(p),
            };

            *current.write() = post;
            *disabled.write() = false;

            if let Ok(Some(p)) = guard.peek().await {
                if let Some(href) = &p.file.url {
                    let kind = match p.file.ext {
                        PostFileExtension::Swf => "embed",
                        PostFileExtension::WebM => "video",
                        _ => "image",
                    };
                    *preload.write() = Preload {
                        href: href.into(),
                        kind: kind.into(),
                    };
                }
            }
        }
    });

    let fav_search_clone = search.clone();
    let like_search_clone = search.clone();
    let dislike_search_clone = search.clone();

    let viewer = match current.as_ref().map(|c| c.file.ext) {
        Some(PostFileExtension::WebM) => rsx! {
            video {
                class: "viewer",
                autoplay: "true",
                muted: "true",
                controls: "true",
                "loop": "true",
                "controlslist": "nofullscreen noremoteplayback",
                "disableremoteplayback": "true",
                src: format_args!(
                    "{}",
                    current
                        .as_ref()
                        .and_then(|p| p.file.url.as_deref().map(str::to_owned))
                        .unwrap_or_else(|| "data:;".to_string()),
                )
            }
        },
        _ => rsx! {
            img {
                class: "viewer",
                src: format_args!(
                    "{}",
                    current
                        .as_ref()
                        .and_then(|p| p.file.url.as_deref().map(str::to_owned))
                        .unwrap_or_else(|| "data:;".to_string()),
                )
            }
        },
    };

    let fav = Rc::new(RefCell::new(move || {
        disabled.set(true);
        if let Some(post) = &*current.read() {
            let post_id = post.id;
            let clone = fav_search_clone.clone();
            let clone2 = fav_search_clone.clone();
            spawn_forever(async move {
                clone.yiff.vote_up(post_id).await.ok();
            });
            spawn_forever(async move {
                clone2.yiff.favorite(post_id).await.unwrap();
            });
        }
        advance.restart()
    }));

    let fav_clone = fav.clone();

    let like = Rc::new(RefCell::new(move || {
        disabled.set(true);
        if let Some(post) = &*current.read() {
            let post_id = post.id;
            let clone = like_search_clone.clone();
            spawn_forever(async move {
                clone.yiff.vote_up(post_id).await.ok();
            });
        }
        advance.restart()
    }));

    let like_clone = like.clone();

    let dislike = Rc::new(RefCell::new(move || {
        disabled.set(true);
        if let Some(post) = &*current.read() {
            let post_id = post.id;
            let clone = dislike_search_clone.clone();
            spawn_forever(async move {
                clone.yiff.unfavorite(post_id).await.ok();
            });
            let clone = dislike_search_clone.clone();
            spawn_forever(async move {
                clone.yiff.vote_down(post_id).await.ok();
            });
        }
        advance.restart()
    }));
    let dislike_clone = dislike.clone();

    let rewind = Rc::new(RefCell::new(move || {
        disabled.set(true);

        let search_clone = search.clone();

        spawn(async move {
            let mut guard = search_clone.search.lock().await;
            let mut reply = guard.prev().await;

            if let Ok(None) = reply {
                reply = guard.next().await;
            }

            let post = match &reply {
                Err(_) => None,   // TODO
                Ok(None) => None, // TODO
                Ok(Some(p)) => Some(p),
            };

            *current.write() = post.cloned();
            *disabled.write() = false;
        });
    }));
    let rewind_clone = rewind.clone();

    let logo_e621 = LOGO_E621.as_str();

    let sources_current_read = current.read();
    let sources = sources_current_read
                        .iter()
                        .map(|post| rsx! {
                            li {
                                ExternalLink { href: Url::parse(&format!("{}/posts/{}", BASE_URL, post.id)).unwrap(),

                                    img { src: "{logo_e621}", alt: "e621 logo" }
                                }
                            }
                        });

    let other_sources = sources_current_read
        .iter()
        .flat_map(|post| &post.sources)
        .filter_map(|source| {
            let href = Url::parse(source).ok()?;
            let host = href.host_str()?.to_string();
            let result = rsx! {
                li {
                    ExternalLink { href, "{host}" }
                }
            };
            Some(result)
        });

    let viewer_style = include_str!("viewer.css");
    let hand_class = match *hand.read() {
        Hand::Left => "left",
        Hand::Right => "right",
    };
    rsx! {
        style { "{viewer_style}" }
        link {
            rel: "preload",
            href: "{preload.read().href}",
            "as": "{preload.read().kind}"
        }
        div {
            prevent_default: "onkeyup",
            onkeyup: move |evt| {
                evt.stop_propagation();
                match evt.data.key() {
                    Key::ArrowRight => like_clone.borrow_mut()(),
                    Key::ArrowDown => dislike_clone.borrow_mut()(),
                    Key::ArrowLeft => rewind_clone.borrow_mut()(),
                    Key::ArrowUp => fav_clone.borrow_mut()(),
                    _ => {}
                }
            },
            id: "viewport-wrapper",
            tabindex: "0",
            "autofocus": "true",
            style: "width: 100%; overflow-x: hidden;",
            div { id: "viewport", {viewer} }
            nav { class: "side-nav {hand_class}",
                ul {
                    li {
                        button {
                            onclick: move |_| fav.borrow_mut()(),
                            tabindex: "-1",
                            disabled: "{disabled}",
                            title: "favorite",
                            div { class: "shortcut", "(⬆)" }
                            "❤️"
                        }
                    }
                    li {
                        button {
                            onclick: move |_| like.borrow_mut()(),
                            tabindex: "-1",
                            disabled: "{disabled}",
                            title: "like",
                            div { class: "shortcut", "(➡)" }
                            "👍"
                        }
                    }
                    li {
                        button {
                            onclick: move |_| dislike.borrow_mut()(),
                            tabindex: "-1",
                            title: "dislike",
                            disabled: "{disabled}",
                            div { class: "shortcut", "(⬇)" }
                            "👎"
                        }
                    }
                    li {
                        button {
                            onclick: move |_| rewind.borrow_mut()(),
                            tabindex: "-1",
                            title: "rewind",
                            disabled: "{disabled}",
                            div { class: "shortcut", "(⬅)" }
                            "◀️"
                        }
                    }
                }
            }

            nav { class: "exit-nav {hand_class}",
                ul {
                    li {
                        button {
                            tabindex: "-1",
                            onclick: move |_| query.write().active = false,
                            "❌"
                        }
                    }
                }
            }

            div { class: "details",

                ul { class: "sources", { sources } }

                ul { class: "other-sources", { other_sources } }
            }
        }
    }
}

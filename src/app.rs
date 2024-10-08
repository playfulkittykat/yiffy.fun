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
use bevy_pkv::PkvStore;

use crate::yiff::Yiff;

use dioxus::prelude::*;
use keyboard_types::Key;

use rs621::post::{Post, PostFileExtension, Query};

use std::sync::Arc;

use futures::lock::Mutex;

use url::Url;

#[cfg(feature = "desktop")]
use dioxus_desktop::use_eval;

#[cfg(feature = "web")]
use dioxus_web::use_eval;

const BASE_URL: &str = "https://e621.net";

lazy_static::lazy_static! {
    static ref LOGO_E621: String = format!(
        "data:image/svg+xml;base64,{}",
        base64::encode(include_bytes!("e621.svg")),
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

#[derive(Props, Clone, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
struct Credentials {
    username: String,
    api_key: String,
    active: bool,
}

impl Credentials {
    fn store() -> PkvStore {
        PkvStore::new_with_qualifier("fun", "yiffy", env!("CARGO_PKG_NAME"))
    }

    async fn load() -> Self {
        // TODO: Find a spawn_blocking replacement.
        Self::store().get("credentials").unwrap_or_default()
    }

    async fn save(self) {
        // TODO: Find a spawn_blocking replacement.
        Self::store().set("credentials", &self).unwrap()
    }
}

pub(crate) fn app(cx: Scope) -> Element {
    // Prevent scrolling with keyboard:
    use_eval(cx)(
        r##"window.addEventListener(
        'keydown',
        (e) => {
            if (e.target.matches("#viewport-wrapper, #viewport-wrapper *")) {
                e.preventDefault()
            }
        }
    )"##
        .to_owned(),
    );

    let credentials = use_future(cx, (), |_| Credentials::load());
    let credentials = credentials.value()?;
    let credentials = use_state(&cx, || credentials.clone());

    let query = use_state(&cx, || String::new());

    if !credentials.active {
        return cx.render(rsx! {
            crate::app::login { credentials: credentials }
            crate::app::notice {}
        });
    }

    if query.is_empty() {
        return cx.render(rsx! {
            crate::app::search { query: query }
            button {
                tabindex: "-1",
                onclick: move |_| {
                    credentials.set(Default::default());
                    cx.spawn_forever(Credentials::default().save());
                },
                "Log Out",
            }
            crate::app::notice {}
        });
    }

    cx.render(rsx! {
        crate::app::viewer {
            credentials: credentials,
            query: query,
        }
    })
}

#[inline_props]
fn external_link<'a>(cx: Scope<'a>, href: Url, children: Element<'a>) -> Element {
    cx.render(rsx! {
        a {
            prevent_default: "onclick",
            onclick: move |_| { webbrowser::open(href.as_str()).ok(); },
            href: "{href}",
            target: "_blank",
            children
        }
    })
}

fn notice(cx: Scope) -> Element {
    let year = &env!("VERGEN_GIT_COMMIT_TIMESTAMP")[..4];
    let source = Url::parse(concat!(env!("CARGO_PKG_REPOSITORY"), "/"))
        .unwrap()
        .join("commit/")
        .unwrap()
        .join(env!("VERGEN_GIT_SHA"))
        .unwrap();

    let license = env!("CARGO_PKG_LICENSE");

    cx.render(rsx! {
        footer {
            class: "copyright",

            style { include_str!("notice.css") },
            "Copyright {year}. "
            "Available under the terms of {license}. "
            crate::app::external_link { href: source, "Source available" }
            "."
        }
    })
}

#[inline_props]
fn search<'a>(cx: Scope, query: &'a UseState<String>) -> Element {
    let partial_query = use_state(&cx, || String::new());

    cx.render(rsx! {
        form {
            prevent_default: "onsubmit",
            onsubmit: move |_| query.set(partial_query.get().clone()),
            input {
                "type": "text",
                value: "{partial_query}",
                placeholder: "tags...",
                oninput: move |evt| partial_query.set(evt.value.clone()),
            }
            button {
                "type": "submit",
                "Start",
            }
        }
    })
}

#[inline_props]
fn login<'a>(cx: Scope, credentials: &'a UseState<Credentials>) -> Element {
    cx.render(rsx! {
        style { include_str!("login.css") }

        div {
            style: "display: inline-block; width: min-content;",

            form {
                prevent_default: "onsubmit",
                onsubmit: move |_| {
                    let mut creds = credentials.make_mut();
                    creds.active = true;
                    cx.spawn_forever(creds.clone().save());
                },

                label {
                    "Username:"
                    input {
                        "type": "text",
                        value: "{credentials.username}",
                        oninput: move |evt| {
                            credentials.make_mut().username = evt.value.clone();
                        }
                    }
                }

                label {
                    "API Key:"
                    input {
                        "type": "password",
                        value: "{credentials.api_key}",
                        minlength: "24",
                        oninput: move |evt| {
                            credentials.make_mut().api_key = evt.value.clone();
                        }
                    }
                }

                button {
                    "type": "submit",
                    "Log In",
                }
            }

            div {
                class: "help",

                "Your API Key is ",
                strong { "not" },
                " your password. You can find your API Key under ",
                strong { "Account > Manage API Access" },
                " once logged into e621.",
            }
        }
    })
}

#[derive(Clone, Debug, Default)]
struct Preload {
    href: String,
    kind: String,
}

#[inline_props]
fn viewer<'a>(
    cx: Scope<'a>,
    credentials: &'a UseState<Credentials>,
    query: &'a UseState<String>,
) -> Element {
    let search = use_future(
        &cx,
        (&(*credentials).clone(), &(*query).clone()),
        |(creds, query)| async move {
            // let yiff = Yiff::new("https://e926.net", "pkk@tabby.rocks");
            let yiff = Yiff::new(BASE_URL, "pkk@tabby.rocks", &creds.username, &creds.api_key);

            let faved = format!("-favoritedby:{}", creds.username);

            // TODO: Verify that this is how I should be splitting query terms.
            let mut query_terms: Vec<_> = str::split(&query, " ")
                .filter_map(|p| match p.trim() {
                    "" => None,
                    rest => Some(rest),
                })
                .collect();

            query_terms.push("order:random");
            query_terms.push("score:>=0");
            query_terms.push("-voted:anything");
            query_terms.push("-type:swf");
            query_terms.push(&faved);

            Search::new(yiff, query_terms.as_slice())
        },
    );

    let search = search.value()?.clone();

    let disabled = use_state(&cx, || false);
    let current = use_state(&cx, || Option::<Arc<Post>>::None).clone();
    let preload = use_state(&cx, Preload::default).clone();

    let current_setter = current.setter();
    let disabled_setter = disabled.setter();
    let disabled_setter_clone = disabled.setter().clone();
    let preload_setter = preload.setter();
    let search_clone = search.clone();
    let advance = use_future(&cx, (), |_| async move {
        let mut guard = search_clone.search.lock().await;
        let reply = guard.next().await;

        let post = match reply {
            Err(_) => None,   // TODO
            Ok(None) => None, // TODO
            Ok(Some(p)) => Some(p),
        };

        current_setter(post);
        disabled_setter(false);

        if let Ok(Some(p)) = guard.peek().await {
            if let Some(href) = &p.file.url {
                let kind = match p.file.ext {
                    PostFileExtension::Swf => "embed",
                    PostFileExtension::WebM => "video",
                    _ => "image",
                };
                let preload = Preload {
                    href: href.into(),
                    kind: kind.into(),
                };
                preload_setter(preload);
            }
        }
    });

    let fav_search_clone = search.clone();
    let fav_current_clone = current.clone();

    let like_search_clone = search.clone();
    let like_current_clone = current.clone();

    let dislike_search_clone = search.clone();
    let dislike_current_clone = current.clone();

    let viewer_current_clone = current.clone();

    let sources_current_clone = current.clone();

    let viewer = match current.as_ref().map(|c| &c.file.ext) {
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
                    viewer_current_clone
                        .as_ref()
                        .and_then(|p| p.file.url.as_deref())
                        .unwrap_or("data:;")
                ),
            }
        },
        _ => rsx! {
            img {
                class: "viewer",
                src: format_args!(
                    "{}",
                    viewer_current_clone
                        .as_ref()
                        .and_then(|p| p.file.url.as_deref())
                        .unwrap_or("data:;")
                ),
            }
        },
    };

    let fav = Arc::new(move || {
        disabled.set(true);
        if let Some(post) = fav_current_clone.as_ref().cloned() {
            let post_id = post.id;
            let clone = fav_search_clone.clone();
            let clone2 = fav_search_clone.clone();
            cx.spawn_forever(async move {
                clone.yiff.vote_up(post_id).await.ok();
            });
            cx.spawn_forever(async move {
                clone2.yiff.favorite(post_id).await.unwrap();
            });
        }
        advance.restart()
    });

    let fav_clone = fav.clone();

    let like = Arc::new(move || {
        disabled.set(true);
        if let Some(post) = like_current_clone.as_ref().cloned() {
            let post_id = post.id;
            let clone = like_search_clone.clone();
            cx.spawn_forever(async move {
                clone.yiff.vote_up(post_id).await.ok();
            });
        }
        advance.restart()
    });

    let like_clone = like.clone();

    let dislike = Arc::new(move || {
        disabled.set(true);
        if let Some(post) = dislike_current_clone.as_ref().cloned() {
            let post_id = post.id;
            let clone = dislike_search_clone.clone();
            cx.spawn_forever(async move {
                clone.yiff.vote_down(post_id).await.ok();
            });
        }
        advance.restart()
    });
    let dislike_clone = dislike.clone();

    let rewind = Arc::new(move || {
        disabled.set(true);

        let disabled_setter_clone = disabled_setter_clone.clone();
        let current_setter = current.setter();
        let search_clone = search.clone();

        cx.spawn(async move {
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

            current_setter(post.cloned());
            disabled_setter_clone(false);
        })
    });
    let rewind_clone = rewind.clone();

    let logo_e621 = LOGO_E621.as_str();

    cx.render(rsx! (
        style { include_str!("viewer.css") }
        link {
            rel: "preload",
            href: "{preload.href}",
            "as": "{preload.kind}",
        }
        div {
            prevent_default: "onkeyup",
            onkeyup: move |evt| {
                evt.stop_propagation();
                match evt.data.key() {
                    Key::ArrowUp => like_clone(),
                    Key::ArrowDown => dislike_clone(),
                    Key::ArrowLeft => rewind_clone(),
                    Key::Character(c) if c == " " => fav_clone(),
                    _ => (),
                }
            },
            id: "viewport-wrapper",
            tabindex: "0",
            "autofocus": "true",
            style: "width: 100%; overflow-x: hidden;",
            div {
                id: "viewport",
                viewer,
            }
            nav {
                class: "side-nav",
                ul {
                    li {
                        button {
                            onclick: move |_| fav(),
                            tabindex: "-1",
                            disabled: "{disabled}",
                            title: "favorite",
                            div {
                                class: "shortcut",
                                "(space)",
                            }
                            "❤️",
                        }
                    }
                    li {
                        button {
                            onclick: move |_| like(),
                            tabindex: "-1",
                            disabled: "{disabled}",
                            title: "like",
                            div {
                                class: "shortcut",
                                "(⬆)",
                            }
                            "👍",
                        }
                    }
                    li {
                        button {
                            onclick: move |_| dislike(),
                            tabindex: "-1",
                            title: "dislike",
                            disabled: "{disabled}",
                            div {
                                class: "shortcut",
                                "(⬇)",
                            }
                            "👎",
                        }
                    }
                    li {
                        button {
                            onclick: move |_| rewind(),
                            tabindex: "-1",
                            title: "rewind",
                            disabled: "{disabled}",
                            div {
                                class: "shortcut",
                                "(⬅)",
                            }
                            "◀️"
                        }
                    }
                }
            }

            nav {
                class: "exit-nav",
                ul {
                    li {
                        button {
                            tabindex: "-1",
                            onclick: move |_| query.set(String::new()),
                            "❌",
                        }
                    }
                }
            }

            div {
                class: "details",

                ul {
                    class: "sources",

                    sources_current_clone
                        .iter()
                        .map(|post| rsx! {
                            li {
                                external_link{href:Url::parse(&format!(
                                        "{}/posts/{}",
                                        BASE_URL,
                                        post.id
                                    )).unwrap(),

                                    img {
                                        src: "{logo_e621}",
                                        alt: "e621 logo",
                                    }
                                }
                            }
                        }),
                }

                ul {
                    class: "other-sources",

                    sources_current_clone
                        .iter()
                        .flat_map(|post| &post.sources)
                        .filter_map(|source| {
                            let href = Url::parse(source).ok()?;
                            let host = href.host_str()?.to_string();
                            let result = rsx! {
                                li {
                                    external_link{
                                        href: href,
                                        "{host}"
                                    }
                                }
                            };
                            Some(result)
                        })
                }
            }
        }
    ))
}

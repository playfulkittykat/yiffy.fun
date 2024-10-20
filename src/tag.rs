/*
 * Yiffy.Fun
 *
 * Copyright (C) 2024 Playful KittyKat
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

use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use dioxus::prelude::*;

use crate::{timers, yiff::Yiff};

static ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entries(Signal<HashMap<usize, String>>);

impl Entries {
    pub fn new() -> Self {
        Self(use_signal(|| {
            let mut map = HashMap::<usize, String>::new();
            map.insert(ID.fetch_add(1, Ordering::SeqCst), String::new());
            map
        }))
    }
}

#[component]
pub fn List(
    yiff: ReadOnlySignal<Yiff>,
    onsubmit: EventHandler<Vec<String>>,
    entries: Entries,
) -> Element {
    let mut entries = entries.0;
    let entries_value = entries.read();
    let mut children = entries_value
        .iter()
        .map(|(id, _)| {
            let id = *id;
            let elem = rsx! {
                Entry {key: "entry-{id}",
                    id,
                    entries,
                    yiff,
                    onremove: move |_| {
                        let mut entries = entries.write();
                        entries.get_mut(&id).unwrap().clear();
                        let rightmost = entries.keys().max().unwrap();
                        if id != *rightmost {
                            entries.remove(&id);
                        }
                    },
                    onchange: move |e| {
                        let mut entries = entries.write();
                        *entries.get_mut(&id).unwrap() = e;
                        let mut rightmost = *entries.keys().max().unwrap();
                        if !entries[&rightmost].is_empty() {
                            rightmost = ID.fetch_add(1, Ordering::SeqCst);
                            entries.insert(rightmost, String::new());
                        }
                        spawn_forever(async move {
                            timers::cancelable(Duration::from_millis(100)).1.await.unwrap();
                            eval(&format!(r#"document.getElementById("tag-edit-{rightmost}").focus();"#))
                                .join()
                                .await
                                .unwrap();
                        });
                    }
                }
            };
            (id, elem)
        })
        .collect::<Vec<_>>();
    children.sort_by_key(|(id, _)| *id);

    let search_style = include_str!("tag.css");
    return rsx! {
        style { "{search_style}" }
        div { class: "tag-list",
            "Tags:"
            { children.into_iter().map(|(_, e)| e) }
        }
        button {
            r#type: "button",
            class: "tag-submit",
            onclick: move |_| {
                let tags: Vec<_> = entries
                    .peek()
                    .values()
                    .filter_map(|x| {
                        match x.trim() {
                            "" => None,
                            x => Some(x.to_owned()),
                        }
                    })
                    .collect();
                onsubmit.call(tags);
            },
            "Show me the Yiff!"
        }
    };
}

#[component]
fn Entry(
    yiff: ReadOnlySignal<Yiff>,
    onremove: EventHandler,
    onchange: EventHandler<String>,
    id: usize,
    entries: Signal<HashMap<usize, String>>,
) -> Element {
    let mut value = use_signal(|| entries.peek().get(&id).cloned().unwrap_or_default());

    rsx! {
        div { class: "tag-entry",
            Edit { id, yiff, onremove, onchange, value }

            Remove {
                onremove: move |_| {
                    value.write().clear();
                    onremove.call(());
                }
            }
        }
    }
}

#[component]
fn Remove(onremove: EventHandler) -> Element {
    rsx! {
        button {
            onclick: move |_| onremove.call(()),
            r#type: "button",
            class: "tag-remove",
            title: "Remove",
            "aria-label": "Remove",
            "X"
        }
    }
}

#[component]
fn Edit(
    yiff: ReadOnlySignal<Yiff>,
    value: Signal<String>,
    onremove: EventHandler,
    onchange: EventHandler<String>,
    id: usize,
) -> Element {
    let mut autocomplete_timer = use_signal(|| Option::<timers::Cancel>::None);
    let mut autocomplete_suggestions = use_signal(Vec::<String>::new);

    let mut chosen = move || {
        use_future(move || {
            eval(&format!(
                r#"document.getElementById("tag-edit-{id}").blur();"#
            ))
            .join()
        });

        let text = value.peek();
        if text.is_empty() {
            autocomplete_suggestions.write().clear();
            onremove.call(());
        } else {
            onchange.call(text.to_owned());
        }
    };

    let oninput = move |e: Event<FormData>| {
        let mut cancel = autocomplete_timer.write();
        if let Some(cancel) = cancel.take() {
            cancel.cancel();
        }
        match value.try_write() {
            Ok(mut v) => *v = e.value(),
            Err(_) => return,
        }

        let text = e.value();
        if text.len() < 3 {
            return;
        }

        let (new_cancel, future) = timers::cancelable(Duration::from_millis(400));
        *cancel = Some(new_cancel);

        spawn_forever(async move {
            if future.await.is_err() {
                return;
            }

            let text = value.peek();
            let (negate, text) = match text.trim_start().chars().next() {
                None => return,
                Some('-') => (true, text.trim_start()[1..].to_owned()),
                Some(_) => (false, text.to_owned()),
            };

            match yiff.read().tags(text).await {
                Err(e) => println!("{}", e),
                Ok(tags) => {
                    let mut suggestions = autocomplete_suggestions.write();
                    if negate {
                        *suggestions = tags.into_iter().map(|x| format!("-{}", x)).collect();
                    } else {
                        *suggestions = tags;
                    }
                }
            }
        });
    };

    rsx! {
        input {
            r#type: "text",
            id: "tag-edit-{id}",
            class: "tag-edit",
            list: "tag-edit-list-{id}",
            value: "{value}",
            oninput,
            onchange: move |_| chosen(),
            onblur: move |_| chosen()
        }
        datalist { id: "tag-edit-list-{id}",

            for suggestion in &*autocomplete_suggestions.read() {
                option { "{suggestion}" }
            }
        }
    }
}

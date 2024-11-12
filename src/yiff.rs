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
use crate::platform::spawn;

use futures::channel::oneshot::Canceled;
use futures::channel::{mpsc, oneshot};
use futures::stream::StreamExt;
use futures::{SinkExt, TryStreamExt};

use rs621::client::Client;
use rs621::post::{Post, Query, VoteDir, VoteMethod};
use rs621::tag;

use snafu::{Backtrace, IntoError, ResultExt, Snafu};

use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("While trying to {action}, encountered: {source}"))]
    Api {
        source: rs621::error::Error,
        action: String,
        backtrace: Backtrace,
    },
    #[snafu(display("While waiting for {action}, encountered: {source}"))]
    NoReply {
        source: Canceled,
        action: String,
        backtrace: Backtrace,
    },
}

#[derive(Clone, Debug)]
pub struct Yiff {
    client: Arc<Client>,
}

impl Yiff {
    pub fn new<U, K>(base_url: &str, creator: &str, username: U, api_key: K) -> Self
    where
        U: Into<String>,
        K: Into<String>,
    {
        // TODO: When in a browser, set the `_client` query parameter.
        let mut client = Client::new(
            base_url,
            format!(
                "{}/{} (by {})",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                creator,
            ),
        )
        .unwrap();

        client.login(username.into(), api_key.into());

        Self {
            client: Arc::new(client),
        }
    }

    pub fn search<T>(&self, query: T) -> Search
    where
        T: Into<Query>,
    {
        let query = query.into();
        let client = self.client.clone();
        let (sender, mut receiver) = mpsc::channel::<Envelope>(5);

        let background = async move {
            let mut search = client.post_search(query);

            let mut history = VecDeque::<Arc<Post>>::new();
            let mut index = 0;

            while let Some(Envelope { msg, reply }) = receiver.next().await {
                let post = match msg {
                    Msg::Rewind if history.is_empty() => None,
                    Msg::Rewind if index == 0 => None,
                    Msg::Rewind if index == 1 => {
                        index -= 1;
                        None
                    }
                    Msg::Rewind => {
                        index -= 1;
                        Some(Ok(history[index - 1].clone()))
                    }

                    Msg::Peek | Msg::Advance if index < history.len() => {
                        Some(Ok(history[index].clone()))
                    }
                    Msg::Peek | Msg::Advance => match search.next().await {
                        Some(Ok(p)) => {
                            // TODO: Limit the max history size.
                            let post = Arc::new(p);
                            history.push_back(post.clone());
                            Some(Ok(post))
                        }
                        Some(Err(e)) => Some(Err(e).with_context(|_| ApiSnafu {
                            action: format!("search page {index}"),
                        })),
                        None => None,
                    },
                };

                if msg == Msg::Advance {
                    if let Some(Ok(_)) = post {
                        index += 1;
                    }
                }

                if reply.send(post).is_err() {
                    break;
                }
            }
        };

        spawn(background);

        Search { sender }
    }

    pub async fn favorite(&self, post_id: u64) -> Result<(), Error> {
        self.client
            .post_favorite(post_id)
            .await
            .map(|_| ())
            .with_context(|_| ApiSnafu {
                action: format!("fav #{post_id}"),
            })
    }

    pub async fn unfavorite(&self, post_id: u64) -> Result<(), Error> {
        self.client
            .post_unfavorite(post_id)
            .await
            .with_context(|_| ApiSnafu {
                action: format!("unfav #{post_id})"),
            })
    }

    pub async fn vote_up(&self, post_id: u64) -> Result<(), Error> {
        self.client
            .post_vote(post_id, VoteMethod::Set, VoteDir::Up)
            .await
            .map(|_| ())
            .with_context(|_| ApiSnafu {
                action: format!("vote up #{post_id}"),
            })
    }

    pub async fn vote_down(&self, post_id: u64) -> Result<(), Error> {
        self.client
            .post_vote(post_id, VoteMethod::Set, VoteDir::Down)
            .await
            .map(|_| ())
            .with_context(|_| ApiSnafu {
                action: format!("vote down #{post_id}"),
            })
    }

    pub async fn tags(&self, needle: String) -> Result<Vec<String>, Error> {
        let query = tag::Query::new()
            .fuzzy_name_matches(needle.clone())
            .order(tag::Order::Similarity)
            .per_page(30);

        let tags = self
            .client
            .tag_search(query)
            .take(30)
            .map_ok(|t| t.name)
            .try_collect::<Vec<_>>()
            .await
            .with_context(move |_| ApiSnafu {
                action: format!("find tags matching `{needle}`"),
            })?;

        Ok(tags)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Msg {
    Peek,
    Advance,
    Rewind,
}

#[derive(Debug)]
struct Envelope {
    msg: Msg,
    reply: oneshot::Sender<Option<Result<Arc<Post>, Error>>>,
}

#[derive(Debug)]
pub struct Search {
    sender: mpsc::Sender<Envelope>,
}

impl Search {
    async fn fetch(&mut self, msg: Msg) -> Option<Result<Arc<Post>, Error>> {
        let (reply, receiver) = oneshot::channel();

        if self.sender.send(Envelope { msg, reply }).await.is_err() {
            return None;
        }

        let err = match receiver.await {
            Ok(r) => return r,
            Err(e) => e,
        };

        Some(Err(NoReplySnafu {
            action: "searching".to_owned(),
        }
        .into_error(err)))
    }

    pub async fn peek(&mut self) -> Result<Option<Arc<Post>>, Error> {
        self.fetch(Msg::Peek).await.transpose()
    }

    pub async fn next(&mut self) -> Result<Option<Arc<Post>>, Error> {
        self.fetch(Msg::Advance).await.transpose()
    }

    pub async fn prev(&mut self) -> Result<Option<Arc<Post>>, Error> {
        self.fetch(Msg::Rewind).await.transpose()
    }
}

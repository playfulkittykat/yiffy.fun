use futures::stream::StreamExt;

use rs621::client::Client;
use rs621::error::Error;
use rs621::post::{Post, Query, VoteDir, VoteMethod};

use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Clone, Debug)]
pub struct Yiff {
    client: Arc<RwLock<Client>>,
}

impl Yiff {
    pub fn new(base_url: &str, creator: &str) -> Self {
        // TODO: When in a browser, set the `_client` query parameter.
        let client = Client::new(
            base_url,
            format!(
                "{}/{} (by {})",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                creator,
            ),
        )
        .unwrap();

        Self {
            client: Arc::new(RwLock::new(client)),
        }
    }

    pub async fn login<U, K>(&self, username: U, api_key: K)
    where
        U: Into<String>,
        K: Into<String>,
    {
        self.client
            .write()
            .await
            .login(username.into(), api_key.into());
    }

    pub fn search<T>(&self, query: T) -> Search
    where
        T: Into<Query>,
    {
        let query = query.into();
        let client = self.client.clone();
        let (sender, mut receiver) =
            mpsc::channel::<(Msg, oneshot::Sender<Option<Result<Arc<Post>, Error>>>)>(5);

        let handle = tokio::spawn(async move {
            let guard = client.read().await;
            let mut search = guard.post_search(query);

            let mut history = VecDeque::<Arc<Post>>::new();
            let mut index = 0;

            while let Some((msg, reply)) = receiver.recv().await {
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
                        Some(Err(e)) => Some(Err(e)),
                        None => None,
                    },
                };

                if msg == Msg::Advance {
                    if let Some(Ok(_)) = post {
                        index += 1;
                    }
                }

                if let Err(_) = reply.send(post) {
                    break;
                }
            }
        });

        Search {
            sender,
            handle: Some(handle),
        }
    }

    pub async fn favorite(&self, post_id: u64) -> Result<(), Error> {
        self.client
            .read()
            .await
            .post_favorite(post_id)
            .await
            .map(|_| ())
    }

    pub async fn vote_up(&self, post_id: u64) -> Result<(), Error> {
        self.client
            .read()
            .await
            .post_vote(post_id, VoteMethod::Set, VoteDir::Up)
            .await
            .map(|_| ())
    }

    pub async fn vote_down(&self, post_id: u64) -> Result<(), Error> {
        self.client
            .read()
            .await
            .post_vote(post_id, VoteMethod::Set, VoteDir::Down)
            .await
            .map(|_| ())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Msg {
    Peek,
    Advance,
    Rewind,
}

#[derive(Debug)]
pub struct Search {
    handle: Option<JoinHandle<()>>,
    sender: mpsc::Sender<(Msg, oneshot::Sender<Option<Result<Arc<Post>, Error>>>)>,
}

impl Search {
    async fn fetch(&mut self, msg: Msg) -> Option<Result<Arc<Post>, Error>> {
        let (sender, receiver) = oneshot::channel();

        if let Err(_) = self.sender.send((msg, sender)).await {
            if let Some(handle) = self.handle.take() {
                if let Err(e) = handle.await {
                    if let Ok(reason) = e.try_into_panic() {
                        std::panic::resume_unwind(reason);
                    }
                }
            }

            return None;
        }

        receiver.await.expect("search ended before replying")
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

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

#[derive(Debug)]
pub struct Cancelled;

impl std::error::Error for Cancelled {}

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cancelled")
    }
}

#[cfg(feature = "desktop")]
mod desktop {
    use std::{future::Future, time::Duration};

    use super::Cancelled;

    pub fn cancelable(delay: Duration) -> (Cancel, impl Future<Output = Result<(), Cancelled>>) {
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        let cancel = Cancel { sender };
        let future = async move {
            let sleep = tokio::time::sleep(delay);
            tokio::pin!(sleep);

            tokio::select! {
                _ = &mut sleep => Ok(()),
                result = receiver => match result {
                    Ok(()) => Err(Cancelled),
                    Err(_) => {
                        sleep.await;
                        Ok(())
                    },
                }
            }
        };

        (cancel, future)
    }

    #[derive(Debug)]
    pub struct Cancel {
        sender: tokio::sync::oneshot::Sender<()>,
    }

    impl Cancel {
        pub fn cancel(self) {
            self.sender.send(()).ok();
        }
    }
}

#[cfg(feature = "desktop")]
pub use self::desktop::*;

#[cfg(feature = "web")]
mod web {
    use std::{future::Future, time::Duration};

    use futures::future::Either;

    use super::Cancelled;

    pub fn cancelable(delay: Duration) -> (Cancel, impl Future<Output = Result<(), Cancelled>>) {
        let (sender, receiver) = futures::channel::oneshot::channel::<()>();
        let cancel = Cancel { sender };

        let future = async move {
            let sleep = gloo::timers::future::sleep(delay);

            match futures::future::select(sleep, receiver).await {
                Either::Left(_) => Ok(()),
                Either::Right((Err(_), sleep)) => Ok(sleep.await),
                Either::Right((Ok(()), _)) => Err(Cancelled),
            }
        };

        (cancel, future)
    }

    #[derive(Debug)]
    pub struct Cancel {
        sender: futures::channel::oneshot::Sender<()>,
    }

    impl Cancel {
        pub fn cancel(self) {
            self.sender.send(()).ok();
        }
    }
}

#[cfg(feature = "web")]
pub use self::web::*;

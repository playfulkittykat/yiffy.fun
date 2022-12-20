use crate::app::app;

use std::future::Future;

pub(crate) fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(fut);
}

pub(super) fn main() {
    dioxus::desktop::launch(app);
}

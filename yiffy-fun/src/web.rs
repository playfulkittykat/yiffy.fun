use crate::app::app;

use std::future::Future;

pub(crate) fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(fut);
}

pub(super) fn main() {
    dioxus::web::launch(app);
}

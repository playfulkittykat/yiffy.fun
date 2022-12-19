mod app;
mod yiff;

#[cfg(feature = "desktop")]
#[path = "desktop.rs"]
mod platform;

#[cfg(feature = "web")]
#[path = "web.rs"]
mod platform;

fn main() {
    self::platform::main();
}

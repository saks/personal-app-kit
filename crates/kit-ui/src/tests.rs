use std::cell::RefCell;

use super::{set_warn, warn};

thread_local! {
    static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn record(message: &str) {
    SEEN.with(|seen| seen.borrow_mut().push(message.to_owned()));
}

#[test]
fn set_warn_routes_warnings_to_the_sink() {
    set_warn(record);
    warn("back-button: could not listen");
    SEEN.with(|seen| assert_eq!(*seen.borrow(), ["back-button: could not listen"]));
}

//! `ThemeProvider`: the only Rust that writes `data-theme` on `<html>`.
//!
//! - **Source of truth:** the app's store, behind [`ThemeApi`] (the kit
//!   doesn't know IPC command names). Device-local, never synced.
//! - **Mirror:** two `localStorage` keys ([`MirrorKeys`]), written whenever
//!   the prefs are loaded or saved. Only `js/theme-boot.js` reads them.
//! - **State:** one [`Source`] in a reducer, shared through [`use_theme`].
//!   Effects on it apply `data-theme`, write the mirror and save.

mod dom;

use std::cell::RefCell;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use kit_core::StoragePrefix;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::mirror::MirrorKeys;
use crate::prefs::{Shade, ThemePrefs};
use crate::state::{Pick, SaveQueue, Source, ThemeAction};

/// Why the app's store could not load or save the prefs. The app maps its
/// own IPC error into this; the kit only logs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeApiError(pub String);

impl fmt::Display for ThemeApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ThemeApiError {}

pub type ThemeFuture<T> = Pin<Box<dyn Future<Output = Result<T, ThemeApiError>>>>;

/// The app's persistence for the prefs (e.g. `settings.json` over IPC).
pub trait ThemeApi {
    fn load(&self) -> ThemeFuture<ThemePrefs>;
    fn save(&self, prefs: ThemePrefs) -> ThemeFuture<()>;
}

/// A shared [`ThemeApi`] for the provider's props. Equal when it is the same
/// instance.
#[derive(Clone)]
pub struct ThemeBackend(Rc<dyn ThemeApi>);

impl ThemeBackend {
    pub fn new(api: impl ThemeApi + 'static) -> Self {
        Self(Rc::new(api))
    }
}

impl PartialEq for ThemeBackend {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Reducible for Source {
    type Action = ThemeAction;

    fn reduce(self: Rc<Self>, action: ThemeAction) -> Rc<Self> {
        let next = self.next(action);
        if next == *self { self } else { next.into() }
    }
}

#[derive(Clone, PartialEq)]
pub struct ThemeCtx {
    pub source: Source,
    pub system: Shade,
    dispatch: UseReducerDispatcher<Source>,
}

impl ThemeCtx {
    pub fn choose(&self, pick: Pick) {
        self.dispatch.dispatch(ThemeAction::Choose(pick));
    }
}

#[hook]
pub fn use_theme() -> Option<ThemeCtx> {
    use_context::<ThemeCtx>()
}

#[derive(Properties, PartialEq)]
pub struct ThemeProviderProps {
    pub storage_prefix: StoragePrefix,
    pub api: ThemeBackend,
    pub children: Html,
}

fn save(api: &ThemeBackend, queue: &Rc<RefCell<SaveQueue>>, prefs: ThemePrefs) {
    let Some(first) = queue.borrow_mut().push(prefs) else {
        return;
    };
    let api = api.clone();
    let queue = queue.clone();
    spawn_local(async move {
        let mut next = Some(first);
        while let Some(prefs) = next {
            if let Err(err) = api.0.save(prefs).await {
                // Still applied (and mirrored) for this session.
                dom::warn(&format!("theme: could not save: {err}"));
            }
            next = queue.borrow_mut().done();
        }
    });
}

#[function_component(ThemeProvider)]
pub fn theme_provider(props: &ThemeProviderProps) -> Html {
    let source = use_reducer(|| Source::Boot);
    let system = use_state(dom::os_shade);
    let queue = use_mut_ref(SaveQueue::default);
    let keys = use_memo(props.storage_prefix.clone(), MirrorKeys::new);
    {
        let dispatch = source.dispatcher();
        let api = props.api.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                match api.0.load().await {
                    Ok(saved) => dispatch.dispatch(ThemeAction::Loaded(saved)),
                    Err(err) => {
                        dom::warn(&format!("theme: could not load: {err}"));
                        dispatch.dispatch(ThemeAction::LoadFailed);
                    }
                }
            });
        });
    }
    {
        let system = system.clone();
        use_effect_with((), move |_| {
            dom::watch_os_shade(move |shade| system.set(shade))
        });
    }
    let current = *source;
    use_effect_with((current, *system), |&(current, system)| {
        if let Some(prefs) = current.prefs() {
            dom::apply(prefs.data_theme(system));
        }
    });
    // Only when the resolved shade flips, not on every theme change.
    use_effect_with(current.bars_shade(*system), |&shade| {
        if let Some(shade) = shade {
            dom::set_system_bars(shade);
        }
    });
    let api = props.api.clone();
    use_effect_with(current, move |&current| {
        if let Some(prefs) = current.mirrored() {
            dom::write_mirror(&keys, prefs);
        }
        if let Source::Chosen(prefs) = current {
            save(&api, &queue, prefs);
        }
    });
    let ctx = ThemeCtx {
        source: current,
        system: *system,
        dispatch: source.dispatcher(),
    };
    html! {
        <ContextProvider<ThemeCtx> context={ctx}>
            { props.children.clone() }
        </ContextProvider<ThemeCtx>>
    }
}

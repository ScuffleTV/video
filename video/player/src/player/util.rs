use std::ops::{Deref, DerefMut};

use tokio::sync::mpsc;
use wasm_bindgen::JsCast;

type BoxedCleanup = Box<dyn FnOnce(&web_sys::EventTarget)>;

/// A holder is a wrapper around an event target which implements JsCast.
/// This is because we want to be able to remove the event listeners when the
/// holder is dropped. This is done by calling the cleanup function.
/// This is really convenient because we can just pass the holder around and not
/// worry about removing the event listeners. The cleanup function is only
/// called once.
pub struct Holder<T: JsCast, E = ()> {
	inner: T,
	events: mpsc::Receiver<E>,
	cleanup: Option<BoxedCleanup>,
}

impl<T: JsCast, E> Holder<T, E> {
	pub fn new(inner: T, events: mpsc::Receiver<E>, cleanup: impl FnOnce(&web_sys::EventTarget) + 'static) -> Self {
		Self {
			inner,
			events,
			cleanup: Some(Box::new(cleanup)),
		}
	}

	pub fn events(&mut self) -> &mut mpsc::Receiver<E> {
		&mut self.events
	}
}

impl<T: JsCast, E> Deref for Holder<T, E> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T: JsCast, E> DerefMut for Holder<T, E> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl<T: JsCast, E> Drop for Holder<T, E> {
	fn drop(&mut self) {
		if let Some(cleanup) = self.cleanup.take() {
			cleanup(self.inner.unchecked_ref());
		}
	}
}

macro_rules! register_events {
    ($ob:ident, {
        $(
            $($evt:literal)|+ => $body:expr
        ),* $(,)?
    }) => {
        {
            #[allow(unused_imports)]
            use wasm_bindgen::JsCast;

            let mut handlers = std::collections::VecDeque::new();
            $(
                handlers.push_back((vec![$($evt.to_string()),+], ::wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new($body)));
                $(
                    $ob.add_event_listener_with_callback($evt, handlers.back().unwrap().1.as_ref().unchecked_ref()).unwrap();
                )*
            )*

            move |val: &web_sys::EventTarget| {
                handlers.drain(..).for_each(|(evts, cb)| {
                    for evt in evts {
                        val.remove_event_listener_with_callback(&evt, cb.as_ref().unchecked_ref()).unwrap();
                    }
                });
            }
        }
    };
}

pub(super) use register_events;
use web_sys::window;

pub fn now() -> f64 {
	window().unwrap().performance().unwrap().now()
}

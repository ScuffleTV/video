use std::cell::Cell;
use std::rc::Rc;

use tokio::sync::mpsc;
use web_sys::{Document, HtmlVideoElement, VisibilityState};

use crate::player::util::{register_events, Holder};

pub struct VisibilityDetector {
	_document: Holder<Document>,
	_vid: Holder<HtmlVideoElement>,
	visible: Rc<Cell<bool>>,
	pip: Rc<Cell<bool>>,
}

impl VisibilityDetector {
	pub fn new(vid: HtmlVideoElement) -> Self {
		let document = web_sys::window().unwrap().document().unwrap();
		let visible = Rc::new(Cell::new(document.visibility_state() == VisibilityState::Visible));
		let pip = Rc::new(Cell::new(false));

		let doc_cleanup = register_events!(document, {
			"visibilitychange" => {
				let visible = visible.clone();
				let document = document.clone();
				move |_| {
					visible.set(document.visibility_state() == VisibilityState::Visible);
				}
			},
		});

		let vid_cleanup = register_events!(vid, {
			"enterpictureinpicture" => {
				let pip = pip.clone();
				move |_| {
					pip.set(true);
				}
			},
			"leavepictureinpicture" => {
				let pip = pip.clone();
				move |_| {
					pip.set(false);
				}
			},
		});

		Self {
			visible,
			pip,
			_vid: Holder::new(vid, mpsc::channel(1).1, vid_cleanup),
			_document: Holder::new(document, mpsc::channel(1).1, doc_cleanup),
		}
	}

	pub fn visible(&self) -> bool {
		self.visible.get() || self.pip.get()
	}
}

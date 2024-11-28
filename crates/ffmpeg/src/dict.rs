use std::collections::HashMap;
use std::ffi::{CStr, CString};

use ffmpeg_sys_next::*;

use crate::error::FfmpegError;
use crate::smart_object::SmartPtr;

pub struct Dictionary {
	ptr: SmartPtr<AVDictionary>,
}

/// Safety: `Dictionary` is safe to send between threads.
unsafe impl Send for Dictionary {}

impl Default for Dictionary {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for Dictionary {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let dict = HashMap::<String, String>::from(self);
		dict.fmt(f)
	}
}

impl Clone for Dictionary {
	fn clone(&self) -> Self {
		let mut dict = Self::new();

		Self::clone_from(&mut dict, self);

		dict
	}

	fn clone_from(&mut self, source: &Self) {
		// Safety: av_dict_copy is safe to call
		let ret = unsafe { av_dict_copy(self.as_mut_ptr_ref(), source.as_ptr(), 0) };
		if ret < 0 {
			panic!("failed to clone dictionary: {ret}")
		}
	}
}

pub struct DictionaryBuilder {
	dict: Dictionary,
}

impl DictionaryBuilder {
	pub fn set(mut self, key: &str, value: &str) -> Self {
		self.dict.set(key, value).expect("Failed to set dictionary entry");
		self
	}

	pub fn build(self) -> Dictionary {
		self.dict
	}
}

impl Dictionary {
	pub fn new() -> Self {
		Self {
			// Safety: A null pointer is a valid dictionary, and a valid pointer.
			ptr: unsafe {
				SmartPtr::wrap(std::ptr::null_mut(), |ptr| {
					// Safety: av_dict_free is safe to call
					av_dict_free(ptr)
				})
			},
		}
	}

	pub fn builder() -> DictionaryBuilder {
		DictionaryBuilder { dict: Self::new() }
	}

	/// # Safety
	/// `ptr` must be a valid pointer.
	/// The caller must also ensure that the dictionary is not freed while this
	/// object is alive, and that we don't use the pointer as mutable
	pub unsafe fn from_ptr_ref(ptr: *mut AVDictionary) -> Self {
		// We don't own the dictionary, so we don't need to free it
		Self {
			ptr: SmartPtr::wrap(ptr as _, |_| {}),
		}
	}

	/// # Safety
	/// `ptr` must be a valid pointer.
	pub unsafe fn from_ptr_owned(ptr: *mut AVDictionary) -> Self {
		Self {
			ptr: SmartPtr::wrap(ptr, |ptr| {
				// Safety: av_dict_free is safe to call
				av_dict_free(ptr)
			}),
		}
	}

	pub fn set(&mut self, key: &str, value: &str) -> Result<(), FfmpegError> {
		let key = CString::new(key).expect("Failed to convert key to CString");
		let value = CString::new(value).expect("Failed to convert value to CString");

		// Safety: av_dict_set is safe to call
		let ret = unsafe { av_dict_set(self.ptr.as_mut(), key.as_ptr(), value.as_ptr(), 0) };

		if ret < 0 {
			Err(FfmpegError::Code(ret.into()))
		} else {
			Ok(())
		}
	}

	pub fn get(&self, key: &str) -> Option<String> {
		let key = CString::new(key).expect("Failed to convert key to CString");

		// Safety: av_dict_get is safe to call
		let entry = unsafe { av_dict_get(self.as_ptr(), key.as_ptr(), std::ptr::null_mut(), AV_DICT_IGNORE_SUFFIX) };

		if entry.is_null() {
			None
		} else {
			// Safety: av_dict_get is safe to call
			Some(unsafe { CStr::from_ptr((*entry).value) }.to_string_lossy().into_owned())
		}
	}

	pub fn iter(&self) -> DictionaryIterator {
		DictionaryIterator::new(self)
	}

	pub fn as_ptr(&self) -> *const AVDictionary {
		self.ptr.as_ptr()
	}

	pub fn as_mut_ptr_ref(&mut self) -> &mut *mut AVDictionary {
		self.ptr.as_mut()
	}

	pub fn into_ptr(self) -> *mut AVDictionary {
		self.ptr.into_inner()
	}
}

pub struct DictionaryIterator<'a> {
	dict: &'a Dictionary,
	entry: *mut AVDictionaryEntry,
}

impl<'a> DictionaryIterator<'a> {
	pub fn new(dict: &'a Dictionary) -> Self {
		Self {
			dict,
			entry: std::ptr::null_mut(),
		}
	}
}

impl<'a> Iterator for DictionaryIterator<'a> {
	type Item = (&'a CStr, &'a CStr);

	fn next(&mut self) -> Option<Self::Item> {
		// Safety: av_dict_get is safe to call
		self.entry = unsafe { av_dict_get(self.dict.as_ptr(), &[0] as *const _ as _, self.entry, AV_DICT_IGNORE_SUFFIX) };

		if self.entry.is_null() {
			None
		} else {
			// Safety: av_dict_get is safe to call
			let key = unsafe { CStr::from_ptr((*self.entry).key) };
			// Safety: av_dict_get is safe to call
			let value = unsafe { CStr::from_ptr((*self.entry).value) };

			Some((key, value))
		}
	}
}

impl<'a> IntoIterator for &'a Dictionary {
	type IntoIter = DictionaryIterator<'a>;
	type Item = <DictionaryIterator<'a> as Iterator>::Item;

	fn into_iter(self) -> Self::IntoIter {
		DictionaryIterator::new(self)
	}
}

impl From<HashMap<String, String>> for Dictionary {
	fn from(map: HashMap<String, String>) -> Self {
		let mut dict = Dictionary::new();

		for (key, value) in map {
			if key.is_empty() || value.is_empty() {
				continue;
			}

			dict.set(&key, &value).expect("Failed to set dictionary entry");
		}

		dict
	}
}

impl From<&Dictionary> for HashMap<String, String> {
	fn from(dict: &Dictionary) -> Self {
		dict.into_iter()
			.map(|(key, value)| (key.to_string_lossy().into_owned(), value.to_string_lossy().into_owned()))
			.collect()
	}
}

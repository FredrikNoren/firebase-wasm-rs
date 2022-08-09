mod bindings;

pub use bindings::{
    delete_object, get_download_url, get_storage, ref_, upload_bytes, Ref, Storage, UploadTask,
    UploadTaskSnapshot,
};
use futures::Stream;
use std::{
    cell::RefCell,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use wasm_bindgen::prelude::*;

impl UploadTask {
    pub fn async_iter(&self) -> UploadTaskAsyncIter {
        let waker: Rc<RefCell<Option<Waker>>> = Rc::default();
        let completed: Rc<RefCell<bool>> = Rc::default();
        let snapshot: Rc<RefCell<Option<UploadTaskSnapshot>>> = Rc::default();
        let err: Rc<RefCell<Option<JsValue>>> = Rc::default();

        let on_snapshot = Closure::new(clone!([snapshot, waker], move |js_snapshot| {
            trace!("UploadTask snapshot:");
            console_log!(&js_snapshot);

            *snapshot.borrow_mut() = Some(js_snapshot);

            if let Some(w) = waker.borrow().as_ref() {
                w.wake_by_ref();
            }
        }));
        let on_err = Closure::new(clone!([completed, err, waker], move |js_err| {
            error!("UploadTask error:");
            console_log!(&js_err);

            *err.borrow_mut() = Some(js_err);

            // Complete the stream since we errored
            *completed.borrow_mut() = true;

            if let Some(w) = waker.borrow().as_ref() {
                w.wake_by_ref()
            }
        }));
        let on_complete = Closure::new(clone!([completed, waker], move || {
            trace!("UploadTask completed");

            *completed.borrow_mut() = true;

            // Notify waker
            let waker_borrow = waker.borrow();

            if let Some(w) = waker_borrow.as_ref() {
                w.wake_by_ref();
            }
        }));

        let unsub = self.on(
            "state_changed",
            &on_snapshot,
            Some(&on_err),
            Some(&on_complete),
        );

        UploadTaskAsyncIter {
            _on_snapshot: on_snapshot,
            _on_err: on_err,
            _on_complete: on_complete,
            snapshot,
            err,
            completed,
            waker,
            unsub,
        }
    }
}

pub struct UploadTaskAsyncIter {
    _on_snapshot: Closure<dyn FnMut(UploadTaskSnapshot)>,
    _on_err: Closure<dyn FnMut(JsValue)>,
    _on_complete: Closure<dyn FnMut()>,
    snapshot: Rc<RefCell<Option<UploadTaskSnapshot>>>,
    err: Rc<RefCell<Option<JsValue>>>,
    completed: Rc<RefCell<bool>>,
    waker: Rc<RefCell<Option<Waker>>>,
    unsub: js_sys::Function,
}

impl Drop for UploadTaskAsyncIter {
    fn drop(&mut self) {
        self.unsub.call0(&JsValue::UNDEFINED).unwrap();
    }
}

impl Stream for UploadTaskAsyncIter {
    type Item = Result<UploadTaskSnapshot, JsValue>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("Polling UploadTaskAsyncIter");

        // Update waker
        *self.waker.borrow_mut() = Some(cx.waker().to_owned());

        if *self.completed.borrow() {
            if let Some(err) = self.err.borrow_mut().take() {
                trace!("UploadTaskAsyncIter errored, returning `Poll::Ready(Some(err))`");

                Poll::Ready(Some(Err(err)))
            } else {
                trace!("UploadTaskAsyncIter completed, returning `Poll::Ready(None)`");

                Poll::Ready(None)
            }
        } else if let Some(snapshot) = self.snapshot.borrow_mut().take() {
            trace!(
                "UploadTaskAsyncIter yielded snapshot, returning \
                 `Poll::Ready(Some(snapshot))`"
            );

            Poll::Ready(Some(Ok(snapshot)))
        } else {
            trace!("UploadTaskAsyncIter pending, returning `Poll::Pending`");

            Poll::Pending
        }
    }
}

use shared_defs::{PollResult, Tag};
use std::future::Future;
use std::mem::{transmute, MaybeUninit};
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

extern "C" {
    #[no_mangle]
    // returns *mut Box<dyn Future<Output = u64>>
    fn two() -> u64;
    #[no_mangle]
    fn poll_two(fut: u64, cx: u64, result_out: *mut PollResult);
}

pub struct OurRawWaker {
    data: *const (),
    _vtable: &'static RawWakerVTable,
}

struct HostFuture {
    future_ptr: u64,
}

impl Future for HostFuture {
    type Output = u64;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        println!("polling HostFuture");
        let mut result_out = MaybeUninit::uninit();
        // YOLO
        let raw_waker: &OurRawWaker = unsafe { transmute(cx.waker()) };
        let host_cx = unsafe { Box::from_raw(raw_waker.data as *mut u64) };
        unsafe {
            poll_two(self.future_ptr, *host_cx, result_out.as_mut_ptr());
        }
        println!("HostFuture polled");
        Box::into_raw(host_cx);
        let result = unsafe { result_out.assume_init() };
        match result.tag {
            Tag::Pending => {
                println!("HostFuture pending");
                Poll::Pending
            }
            Tag::Ready => {
                println!("HostFuture ready");
                Poll::from(result.result)
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn async_main() -> u32 {
    async fn body() -> u64 {
        let forty = async { 40u64 };
        let two = HostFuture {
            future_ptr: unsafe { two() },
        };
        forty.await + two.await
    }
    let fut = Box::new(body()) as Box<dyn Future<Output = u64>>;
    // return a *mut Box<dyn Future<Output = u64>> so that the returned pointer is thin
    Box::into_raw(Box::new(fut)) as u32
}

const DUMMY_VTABLE: RawWakerVTable =
    RawWakerVTable::new(dummy_clone, dummy_wake, dummy_wake_by_ref, dummy_drop);

unsafe fn dummy_clone(_: *const ()) -> RawWaker {
    unimplemented!("dummy_clone")
}

unsafe fn dummy_wake(_: *const ()) {
    unimplemented!("dummy_wake")
}

unsafe fn dummy_wake_by_ref(_: *const ()) {
    unimplemented!("dummy_wake_by_ref")
}

unsafe fn dummy_drop(cx: *const ()) {
    // don't leak the boxed host context pointer
    Box::from_raw(cx as *mut u64);
}

#[no_mangle]
pub extern "C" fn poll_future(fut: *mut Box<dyn Future<Output = u64>>, cx: u64) -> u32 {
    let mut fut = unsafe { Box::from_raw(fut) };
    let pin_fut = unsafe { Pin::new_unchecked(fut.deref_mut().deref_mut()) };
    let host_cx = Box::into_raw(Box::new(cx));
    let raw_waker = RawWaker::new(host_cx as *const (), &DUMMY_VTABLE);
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = Context::from_waker(&waker);
    let poll_result = match pin_fut.poll(&mut cx) {
        Poll::Pending => PollResult {
            tag: Tag::Pending,
            result: 0,
        },
        Poll::Ready(result) => PollResult {
            tag: Tag::Ready,
            result,
        },
    };
    let poll_result_ptr = Box::into_raw(Box::new(poll_result));
    // leak the future so it can be called again
    Box::into_raw(fut);
    poll_result_ptr as u32
}

fn main() {
    println!("Hello, world!");
}

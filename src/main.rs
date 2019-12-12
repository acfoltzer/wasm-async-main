use lucet_runtime::{lucet_hostcalls, DlModule, Instance, Limits, MmapRegion, Region, Val};
use shared_defs::{PollResult, Tag};
use std::future::Future;
use std::ops::DerefMut;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

lucet_hostcalls! {
    #[no_mangle]
    pub unsafe extern "C" fn two(&mut _vmctx,) -> *mut AssertUnwindSafe<Box<dyn Future<Output = u64>>> {
        async fn body() -> u64 {
            println!("opening file");
            let mut f = File::open("two.txt").await.unwrap();
            println!("file opened");
            let mut s = String::new();
            f.read_to_string(&mut s).await.unwrap();
            s.trim_end().parse::<u64>().unwrap()
        }
        let fut = Box::new(body()) as Box<dyn Future<Output = u64>>;
        // return a *mut Box<dyn Future<Output = u64>> so that the returned pointer is thin
        Box::into_raw(Box::new(AssertUnwindSafe(fut)))
    }

    #[no_mangle]
    pub unsafe extern "C" fn poll_two(
        &mut vmctx,
        fut: *mut AssertUnwindSafe<Box<dyn Future<Output = u64>>>,
        cx: *mut Context,
        result_out: u32, // *mut PollResult
    ) -> () {
        dbg!(cx);
        let mut fut = Box::from_raw(fut);
        let pin_fut = Pin::new_unchecked(fut.deref_mut().deref_mut().deref_mut());
        let poll_result = match pin_fut.poll(cx.as_mut().unwrap()) {
            Poll::Pending => PollResult {
                tag: Tag::Pending,
                result: 0,
            },
            Poll::Ready(result) => PollResult {
                tag: Tag::Ready,
                result,
            },
        };
        let result_out = &mut vmctx.heap_mut()[result_out as usize] as *mut _ as *mut PollResult;
        result_out.write(poll_result);
    }
}

struct LucetFuture<'a> {
    inst: &'a mut Instance,
    future_ptr: u32,
}

impl<'a> Future for LucetFuture<'a> {
    type Output = Result<u64, lucet_runtime::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let ptr = self.future_ptr.into();
        println!("calling poll_future");
        dbg!(cx as *mut _);
        let res = match self
            .inst
            .run("poll_future", &[ptr, (cx as *mut _ as u64).into()])
        {
            Ok(res) => match res.returned() {
                Ok(val) => {
                    let poll_result = unsafe {
                        &*(&self.inst.heap()[val.as_u32() as usize] as *const _
                            as *const PollResult)
                    };
                    match poll_result.tag {
                        Tag::Ready => Poll::from(Ok(poll_result.result)),
                        Tag::Pending => Poll::Pending,
                    }
                }
                Err(e) => Poll::from(Err(e)),
            },
            Err(e) => Poll::from(Err(e)),
        };
        println!("poll_future returned: {:?}", res);
        res
    }
}

async fn async_run_u64<'a>(
    inst: &'a mut Instance,
    entrypoint: &str,
    args: &[Val],
) -> Result<u64, lucet_runtime::Error> {
    let future_ptr = inst.run(entrypoint, args)?.returned()?.as_u32();
    println!("async main returned");
    LucetFuture { inst, future_ptr }.await
}

#[tokio::main]
async fn main() -> Result<(), lucet_runtime::Error> {
    let mut f = File::open("two.txt").await.unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).await.unwrap();
    println!("{}", s.trim_end().parse::<u64>().unwrap());

    let limits = Limits {
        heap_memory_size: 1024 * 1024 * 1024,
        stack_size: 8 * 1024 * 1024,
        ..Limits::default()
    };
    let region = MmapRegion::create(1, &limits)?;
    let module = DlModule::load("target/guest.so")?;
    let mut inst = region.new_instance(module)?;
    inst.insert_embed_ctx::<lucet_wasi::WasiCtx>(
        lucet_wasi::WasiCtxBuilder::new().inherit_stdio().build()?,
    );
    println!("running async_main");
    let res = async_run_u64(&mut inst, "async_main", &[]).await?;
    println!("final result: {}", res);
    Ok(())
}

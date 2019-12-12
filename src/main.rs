use lucet_runtime::{DlModule, Instance, Limits, MmapRegion, Region, Val};
use shared_defs::{PollResult, Tag};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct LucetFuture<'a> {
    inst: &'a mut Instance,
    future_ptr: u32,
}

impl<'a> Future for LucetFuture<'a> {
    type Output = Result<u64, lucet_runtime::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let ptr = self.future_ptr.into();
        println!("calling poll_future");
        match self
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
        }
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

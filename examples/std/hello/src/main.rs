#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::sync::atomic::AtomicU32;
use drogue_device::*;

mod myactor;
mod mypack;

use myactor::*;
use mypack::*;

pub struct MyDevice {
    counter: AtomicU32,
    a: ActorContext<'static, MyActor>,
    b: ActorContext<'static, MyActor>,
    p: MyPack,
}

#[drogue::main]
async fn main(context: DeviceContext<MyDevice>) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    context.configure(MyDevice {
        counter: AtomicU32::new(0),
        a: ActorContext::new(MyActor::new("a")),
        b: ActorContext::new(MyActor::new("b")),
        p: MyPack::new(),
    });

    let (a_addr, b_addr, c_addr) = context.mount(|device, spawner| {
        let a_addr = device.a.mount(&device.counter, spawner);
        let b_addr = device.b.mount(&device.counter, spawner);
        let c_addr = device.p.mount((), spawner);
        (a_addr, b_addr, c_addr)
    });

    loop {
        time::Timer::after(time::Duration::from_secs(1)).await;
        // Send that completes immediately when message is enqueued
        a_addr.notify(SayHello("World")).unwrap();
        // Send that waits until message is processed
        b_addr.request(SayHello("You")).unwrap().await;

        // Actor uses a different counter
        c_addr.notify(SayHello("There")).unwrap();
    }
}

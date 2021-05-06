#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(incomplete_features)]
#![allow(dead_code)]
#![feature(min_type_alias_impl_trait)]
#![feature(const_generics_defaults)]
#![feature(const_generics)]
#![feature(const_evaluatable_checked)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(associated_type_defaults)]
#![feature(type_alias_impl_trait)]
//! An async, no-alloc actor framework for embedded devices.
//!
//! See [the book](https://book.drogue.io/drogue-device/dev/index.html) for more about the architecture, how to write device drivers, and running some examples.
//!
//! # Actor System
//!
//! An _actor system_ is a framework that allows for isolating state within narrow contexts, making it easier to reason about system.
//! Within a actor system, the primary component is an _Actor_, which represents the boundary of state usage.
//! Each actor has exclusive access to its own state and only communicates with other actors through message-passing.
//!
//! # Example
//!
//! ```
//! #![macro_use]
//! #![allow(incomplete_features)]
//! #![feature(generic_associated_types)]
//! #![feature(min_type_alias_impl_trait)]
//! #![feature(impl_trait_in_bindings)]
//! #![feature(type_alias_impl_trait)]
//! #![feature(concat_idents)]
//!
//! use drogue_device::*;
//!
//! pub struct MyActor {
//!     name: &'static str,
//! }
//!
//! pub struct SayHello<'m>(&'m str);
//!
//! impl MyActor {
//!     pub fn new(name: &'static str) -> Self {
//!         Self { name }
//!     }
//! }
//!
//! impl Actor for MyActor {
//!     type Message<'a> = SayHello<'a>;
//!     type OnStartFuture<'a> = impl core::future::Future<Output = ()> + 'a;
//!     type OnMessageFuture<'a> = impl core::future::Future<Output = ()> + 'a;
//!
//!     fn on_start(self: core::pin::Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
//!         async move { println!("[{}] started!", self.name) }
//!     }
//!
//!     fn on_message<'m>(
//!         self: core::pin::Pin<&'m mut Self>,
//!         message: Self::Message<'m>,
//!     ) -> Self::OnMessageFuture<'m> {
//!         async move {
//!             println!("[{}] Hello {}", self.name, message.0);
//!         }
//!     }
//! }
//!
//! #[derive(Device)]
//! pub struct MyDevice {
//!     a: ActorContext<'static, MyActor>,
//! }
//!
//! #[drogue::main]
//! async fn main(mut context: DeviceContext<MyDevice>) {
//!     context.configure(MyDevice {
//!         a: ActorContext::new(MyActor::new("a")),
//!     });
//!     let a_addr = context.mount(|device| {
//!         device.a.mount(())
//!     });
//!     a_addr.request(SayHello("World")).await;
//! }
//!```
//!

pub(crate) mod fmt;

pub mod kernel;
pub use kernel::{
    actor::{Actor, ActorContext, Address},
    channel::Channel,
    device::{Device, DeviceContext},
    package::{Package, PackageConfig, PackageContext},
    util::ImmediateFuture,
};

pub mod actors;

pub mod traits;

pub mod drivers;

#[doc(hidden)]
pub use drogue_device_macros::{self as drogue, log_stack, Device, Package};
pub use embassy::*;

#[cfg(feature = "chip+nrf52833")]
pub use embassy_nrf as nrf;

#[cfg(feature = "chip+rp")]
pub use embassy_rp as rp;

#[cfg(any(feature = "chip+stm32l0x2", feature = "chip+stm32f401"))]
pub use embassy_stm32 as stm32;

#[doc(hidden)]
pub mod reexport {
    pub use ::embassy;
    #[cfg(feature = "chip+nrf52833")]
    pub use ::embassy_nrf;
    #[cfg(feature = "chip+rp")]
    pub use ::embassy_rp;
    #[cfg(feature = "std")]
    pub use ::embassy_std;
    #[cfg(any(feature = "chip+stm32l0x2", feature = "chip+stm32f401"))]
    pub use ::embassy_stm32;
}

#[doc(hidden)]
#[cfg(feature = "std")]
pub use embassy_std::*;

#[cfg(feature = "std")]
pub mod testutil;

#[allow(unused_variables)]
pub fn print_stack(file: &'static str, line: u32) {
    let _u: u32 = 1;
    let _uptr: *const u32 = &_u;
    // log::trace!("[{}:{}] SP: 0x{:p}", file, line, &_uptr);
}

#[allow(unused_variables)]
pub fn print_size<T>(name: &'static str) {
    //log::info!("[{}] size: {}", name, core::mem::size_of::<T>());
}

#[allow(unused_variables)]
pub fn print_value_size<T>(name: &'static str, val: &T) {
    /*    log::info!(
        "[{}] value size: {}",
        name,
        core::mem::size_of_val::<T>(val)
    );*/
}

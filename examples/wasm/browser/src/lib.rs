#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
mod components;
mod system;

use components::*;
use system::*;

use wasm_bindgen::prelude::*;

use drogue_device::{
    actors::{button::*, led::*},
    *,
};

struct MyDevice {
    led: ActorContext<'static, Led<WebLed>>,
    button: ActorContext<'static, Button<'static, WebButton, Led<WebLed>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

// Called when the wasm module is instantiated
#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::default());
    let spawner = WasmSpawner::new();

    // Configure HTML elements
    unsafe {
        INPUT1.configure("button");
        OUTPUT1.configure("led");
    }

    DEVICE.configure(MyDevice {
        led: ActorContext::new(Led::new(WebLed::new(unsafe { &OUTPUT1 }))),
        button: ActorContext::new(Button::new(WebButton::new(unsafe { &INPUT1 }))),
    });

    DEVICE.mount(|device| {
        let led = device.led.mount((), spawner);
        device.button.mount(led, spawner);
    });

    Ok(())
}

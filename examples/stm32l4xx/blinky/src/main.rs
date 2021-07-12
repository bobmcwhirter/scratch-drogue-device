#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{actors::ticker::*, drivers::led::*, *};
use embassy_stm32::{
    gpio::{Level, Output},
    interrupt,
    peripherals::PB3,
    Peripherals,
};

mod app;

use app::*;
use embassy::time::Duration;
use embassy_stm32::time::U32Ext;
use stm32l4::stm32l4x2 as pac;

type Led1Pin = Output<'static, PB3>;

type MyApp = App<Led1Pin>;

pub struct MyDevice {
    app: ActorContext<'static, MyApp>,
    ticker: ActorContext<'static, Ticker<'static, MyApp>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

fn test() {
    embassy_stm32::Config::default().rcc(
        embassy_stm32::rcc::Config::default()
            .clock_src(embassy_stm32::rcc::ClockSrc::HSE(80.mhz().into())),
    );
}

/*
[embassy::main(config = "embassy_stm32::Config::default().rcc(
    embassy_stm32::rcc::Config::default()
        .clock_src(embassy_stm32::rcc::ClockSrc::HSE(80.mhz().into())),
)")]
*/
/*
#[embassy::main(config = "embassy_stm32::Config::default().rcc(
    embassy_stm32::rcc::Config::default()
        .clock_src(embassy_stm32::rcc::ClockSrc::HSI16),
)")]
 */
#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let pp = pac::Peripherals::take().unwrap();

    pp.DBGMCU.cr.modify(|_, w| {
        w.dbg_sleep().set_bit();
        w.dbg_standby().set_bit();
        w.dbg_stop().set_bit()
    });

    pp.RCC.ahb1enr.modify(|_, w| w.dma1en().set_bit());

    pp.RCC.ahb2enr.modify(|_, w| {
        w.gpioaen().set_bit();
        w.gpioben().set_bit();
        w.gpiocen().set_bit();
        w.gpioden().set_bit();
        w.gpioeen().set_bit();
        w
    });

    defmt::info!("Starting up...");

    let led1 = Led::new(Output::new(p.PB3, Level::High));

    DEVICE.configure(MyDevice {
        ticker: ActorContext::new(Ticker::new(Duration::from_millis(250), Command::Toggle)),
        app: ActorContext::new(App::new(AppInitConfig { user_led: led1 })),
    });

    DEVICE.mount(|device| {
        let app = device.app.mount((), spawner);
        let ticker = device.ticker.mount(app, spawner);
        ticker.notify(TickerCommand::Start).unwrap();
    });
}

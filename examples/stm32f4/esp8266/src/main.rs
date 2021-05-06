#![no_main]
#![no_std]
#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const ENDPOINT: IpAddress = IpAddress::new_v4(192, 168, 1, 2);
const ENDPOINT_PORT: u16 = 12345;

mod app;

use app::*;

use cortex_m_rt::{entry, exception};
use log::LevelFilter;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use drogue_device::{
    actors::button::Button,
    drivers::wifi::esp8266::*,
    stm32::{
        hal::serial::{
            config::{Config, Parity, StopBits},
            Event, Serial,
        },
        hal::stm32::Interrupt,
        Peripherals,
    },
    traits::ip::*,
    *,
};

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

type UART = BufferedUarte<'static, UARTE0, TIMER0>;
type ENABLE = Output<'static, P0_03>;
type RESET = Output<'static, P0_02>;

#[derive(Device)]
pub struct MyDevice {
    driver: UnsafeCell<Esp8266Driver>,
    modem: ActorContext<'static, Esp8266ModemActor<'static, UART, ENABLE, RESET>>,
    app: ActorContext<'static, App<Esp8266Controller<'static>>>,
    button: ActorContext<
        'static,
        Button<'static, PortInput<'static, P0_14>, App<Esp8266Controller<'static>>>,
    >,
}

#[drogue::main]
async fn main(context: DeviceContext<MyDevice>) {
    rtt_init_print!();
    unsafe {
        log::set_logger_racy(&LOGGER).unwrap();
    }
    log::set_max_level(log::LevelFilter::Info);

    let (device, clocks) = Peripherals::take().unwrap();

    let gpioa = device.GPIOA.split();
    let gpioc = device.GPIOC.split();

    let pa11 = gpioa.pa11;
    let pa12 = gpioa.pa12;

    // SERIAL pins for USART6
    let tx_pin = pa11.into_alternate_af8();
    let rx_pin = pa12.into_alternate_af8();

    // enable pin
    let en = gpioc.pc10.into_push_pull_output();
    // reset pin
    let reset = gpioc.pc12.into_push_pull_output();

    let usart6 = device.USART6;

    let mut serial = Serial::usart6(
        usart6,
        (tx_pin, rx_pin),
        Config {
            baudrate: 115_200.bps(),
            parity: Parity::ParityNone,
            stopbits: StopBits::STOP1,
            ..Default::default()
        },
        clocks,
    )
    .unwrap();

    serial.listen(Event::Rxne);
    let (tx, rx) = serial.split();

    let uart = Serial::new(tx, rx, Interrupt::USART6);
    let wifi = Esp8266Wifi::new(en, reset);

    let timer = AppTimer::new(DummyTimer {}, Interrupt::TIM2);

    // Create a button input with an interrupt
    let mut button = gpioc.pc13.into_pull_up_input();
    button.make_interrupt_source(&mut device.SYSCFG.constrain());
    button.enable_interrupt(&mut device.EXTI);
    button.trigger_on_edge(&mut device.EXTI, Edge::FALLING);

    let button = Button::new(button, Active::Low);

    MyDevice {
        button: InterruptContext::new(button, Interrupt::EXTI15_10).with_name("button"),
        timer,
        uart,
        wifi,
        app: ActorContext::new(App::new(WIFI_SSID, WIFI_PSK, ENDPOINT, ENDPOINT_PORT))
            .with_name("app"),
    }
}

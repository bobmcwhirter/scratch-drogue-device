use core::future::Future;
use core::pin::Pin;
use drogue_device::{drivers::led::*, *};
use embedded_hal::digital::v2::{StatefulOutputPin, ToggleableOutputPin};

#[derive(Clone, Copy)]
pub enum Command {
    Toggle,
}

pub struct AppInitConfig<L1>
where
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    pub user_led: Led<L1>,
}

pub struct App<L1>
where
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    config: AppInitConfig<L1>,
}

impl<L1> App<L1>
where
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    pub fn new(config: AppInitConfig<L1>) -> Self {
        Self { config }
    }
}

impl<L1> Unpin for App<L1> where L1: StatefulOutputPin + ToggleableOutputPin {}

impl<L1> Actor for App<L1>
where
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    #[rustfmt::skip]
    type Message<'m> = Command;
    #[rustfmt::skip]
    type OnStartFuture<'m> = impl Future<Output = ()> + 'm;

    fn on_start<'m>(mut self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {
            self.config.user_led.on().ok();
        }
    }

    type OnMessageFuture<'m> = impl Future<Output = ()> + 'm;

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        defmt::info!("Message");
        match message {
            Command::Toggle => {
                self.config.user_led.toggle().ok();
            }
        }
        async {}
    }
}

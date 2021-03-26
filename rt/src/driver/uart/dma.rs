use crate::prelude::*;

pub use crate::api::uart::Error;
use crate::api::{
    timer::*,
    uart::{UartRequest},
};
use crate::domain::time::duration::{Duration, Milliseconds};
use crate::hal::uart::dma::DmaUartHal;

use core::cell::{RefCell, UnsafeCell};
use cortex_m::interrupt::Nr;

use super::common::*;

use crate::util::dma::async_bbqueue::{Error as QueueError, *};

pub struct UartActor<T, TXN, RXN>
where
    T: Timer + 'static,
    TXN: ArrayLength<u8> + 'static,
    RXN: ArrayLength<u8> + 'static,
{
    me: Option<Address<Self>>,
    scheduler: Option<Address<T>>,
    shared: Option<&'static ActorState>,
    rx_consumer: Option<AsyncBBConsumer<RXN>>,
    tx_producer: Option<AsyncBBProducer<TXN>>,
}

pub struct UartController<U>
where
    U: DmaUartHal + 'static,
{
    uart: Option<&'static U>,
}

pub struct UartInterrupt<U, T, TXN, RXN>
where
    U: DmaUartHal + 'static,
    T: Timer + 'static,
    TXN: ArrayLength<u8> + 'static,
    RXN: ArrayLength<u8> + 'static,
{
    scheduler: Option<Address<T>>,
    me: Option<Address<Self>>,
    uart: Option<&'static U>,
    controller: Option<Address<UartController<U>>>,
    tx_consumer: Option<AsyncBBConsumer<TXN>>,
    tx_consumer_grant: Option<RefCell<AsyncBBConsumerGrant<'static, TXN>>>,
    rx_producer: Option<AsyncBBProducer<RXN>>,
    rx_producer_grant: Option<RefCell<AsyncBBProducerGrant<'static, RXN>>>,
}

pub struct DmaUart<U, T, TXN, RXN>
where
    U: DmaUartHal + 'static,
    T: Timer + 'static,
    TXN: ArrayLength<u8> + 'static,
    RXN: ArrayLength<u8> + 'static,
{
    uart: U,
    actor: ActorContext<UartActor<T, TXN, RXN>>,
    controller: ActorContext<UartController<U>>,
    interrupt: InterruptContext<UartInterrupt<U, T, TXN, RXN>>,
    shared: ActorState,

    rx_buffer: UnsafeCell<AsyncBBBuffer<'static, RXN>>,
    rx_cons: RefCell<Option<UnsafeCell<AsyncBBConsumer<RXN>>>>,
    rx_prod: RefCell<Option<UnsafeCell<AsyncBBProducer<RXN>>>>,

    tx_buffer: UnsafeCell<AsyncBBBuffer<'static, TXN>>,
    tx_cons: RefCell<Option<UnsafeCell<AsyncBBConsumer<TXN>>>>,
    tx_prod: RefCell<Option<UnsafeCell<AsyncBBProducer<TXN>>>>,
}

impl<U, T, TXN, RXN> DmaUart<U, T, TXN, RXN>
where
    U: DmaUartHal + 'static,
    T: Timer + 'static,
    TXN: ArrayLength<u8>,
    RXN: ArrayLength<u8>,
{
    pub fn new<IRQ>(uart: U, irq: IRQ) -> Self
    where
        IRQ: Nr,
    {
        Self {
            uart,
            actor: ActorContext::new(UartActor::new()).with_name("uart_actor"),
            controller: ActorContext::new(UartController::new()).with_name("uart_controller"),
            interrupt: InterruptContext::new(UartInterrupt::new(), irq).with_name("uart_interrupt"),
            shared: ActorState::new(),
            rx_buffer: UnsafeCell::new(AsyncBBBuffer::new()),
            rx_prod: RefCell::new(None),
            rx_cons: RefCell::new(None),

            tx_buffer: UnsafeCell::new(AsyncBBBuffer::new()),
            tx_prod: RefCell::new(None),
            tx_cons: RefCell::new(None),
        }
    }
}

impl<U, T, TXN, RXN> Package for DmaUart<U, T, TXN, RXN>
where
    U: DmaUartHal,
    T: Timer + 'static,
    TXN: ArrayLength<u8>,
    RXN: ArrayLength<u8>,
{
    type Primary = UartActor<T, TXN, RXN>;
    type Configuration = Address<T>;
    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let (rx_prod, rx_cons) = unsafe { (&mut *self.rx_buffer.get()).split() };
        let (tx_prod, tx_cons) = unsafe { (&mut *self.tx_buffer.get()).split() };

        let controller = self.controller.mount(&self.uart, supervisor);
        let addr = self
            .actor
            .mount((&self.shared, config, tx_prod, rx_cons), supervisor);
        self.interrupt.mount(
            (&self.uart, controller, config, tx_cons, rx_prod),
            supervisor,
        );

        addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.actor.address()
    }
}

impl<T, TXN, RXN> UartActor<T, TXN, RXN>
where
    T: Timer + 'static,
    TXN: ArrayLength<u8>,
    RXN: ArrayLength<u8>,
{
    pub fn new() -> Self {
        Self {
            shared: None,
            me: None,
            scheduler: None,
            rx_consumer: None,
            tx_producer: None,
        }
    }
}

impl<U> Actor for UartController<U>
where
    U: DmaUartHal,
{
    type Configuration = &'static U;
    type Request = RxTimeout;
    type Response = ();

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.uart.replace(config);
    }

    fn on_request(self, message: RxTimeout) -> Response<Self> {
        let uart = self.uart.as_ref().unwrap();
        uart.cancel_read();
        Response::immediate(self, ())
    }
}

impl<U> UartController<U>
where
    U: DmaUartHal,
{
    pub fn new() -> Self {
        Self { uart: None }
    }
}

impl<'a, T, TXN, RXN> Actor for UartActor<T, TXN, RXN>
where
    T: Timer + 'static,
    TXN: ArrayLength<u8>,
    RXN: ArrayLength<u8>,
{
    type Configuration = (
        &'static ActorState,
        Address<T>,
        AsyncBBProducer<TXN>,
        AsyncBBConsumer<RXN>,
    );

    type Request = UartRequest<'a>;
    type Response = UartResponse;

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.me.replace(me);
        self.shared.replace(config.0);
        self.scheduler.replace(config.1);
        self.tx_producer.replace(config.2);
        self.rx_consumer.replace(config.3);
    }

    fn on_request(self, request: UartRequest<'a>) -> Response<Self, Result<usize, Error>> {
        match request {
            // Read bytes into the provided rx_buffer. The memory pointed to by the buffer must be available until the return future is await'ed
            UartRequest::Read(rx_buf) => {
        let shared = self.shared.as_ref().unwrap();
        if shared.try_rx_busy() {
            let rx_consumer = self.rx_consumer.as_ref().unwrap();
            let future = unsafe { rx_consumer.read(rx_buf) };
            let future = RxFuture::new(future, shared);
            Response::immediate_future(self, future)
        } else {
            Response::immediate(self, Err(Error::RxInProgress))
        }
            }
            // Transmit bytes from provided tx_buffer over UART. The memory pointed to by the buffer must be available until the return future is await'ed
            UartRequest::Write(tx_buf) => {
        let shared = self.shared.as_ref().unwrap();
        if shared.try_tx_busy() {
            // log::info!("Going to write message");
            let tx_producer = self.tx_producer.as_ref().unwrap();
            let future = unsafe { tx_producer.write(tx_buf) };
            let future = TxFuture::new(future, shared);
            Response::immediate_future(self, future)
        } else {
            Response::immediate(self, Err(Error::TxInProgress))
        }
            }
    }
    }
}

impl<U, T, TXN, RXN> UartInterrupt<U, T, TXN, RXN>
where
    U: DmaUartHal,
    T: Timer + 'static,
    TXN: ArrayLength<u8>,
    RXN: ArrayLength<u8>,
{
    pub fn new() -> Self {
        Self {
            uart: None,
            tx_consumer: None,
            rx_producer: None,
            tx_consumer_grant: None,
            rx_producer_grant: None,
            me: None,
            scheduler: None,
            controller: None,
        }
    }

    fn start_write(&mut self) {
        let uart = self.uart.as_ref().unwrap();
        let tx_consumer = self.tx_consumer.as_ref().unwrap();
        match tx_consumer.prepare_read() {
            Ok(grant) => match uart.prepare_write(grant.buf()) {
                Ok(_) => {
                    self.tx_consumer_grant.replace(RefCell::new(grant));
                    // log::info!("Starting WRITE");
                    uart.start_write();
                }
                Err(e) => {
                    log::error!("Error preparing write, backing off: {:?}", e);
                    self.scheduler.as_ref().unwrap().schedule(
                        Milliseconds(1000),
                        DmaRequest::TxStart,
                        *self.me.as_ref().unwrap(),
                    );
                }
            },
            Err(QueueError::BufferEmpty) => {
                // TODO: Go to sleep
                self.scheduler.as_ref().unwrap().schedule(
                    Milliseconds(10),
                    DmaRequest::TxStart,
                    *self.me.as_ref().unwrap(),
                );
            }
            Err(e) => {
                log::error!("Error pulling from queue, backing off: {:?}", e);
                self.scheduler.as_ref().unwrap().schedule(
                    Milliseconds(1000),
                    DmaRequest::TxStart,
                    *self.me.as_ref().unwrap(),
                );
            }
        }
    }

    fn start_read(&mut self, read_size: usize, timeout: Milliseconds) {
        let uart = self.uart.as_ref().unwrap();
        let rx_producer = self.rx_producer.as_ref().unwrap();
        // TODO: Handle error?
        match rx_producer.prepare_write(read_size) {
            Ok(mut grant) => match uart.prepare_read(grant.buf()) {
                Ok(_) => {
                    self.rx_producer_grant.replace(RefCell::new(grant));
                    uart.start_read();
                    if timeout > Milliseconds(0_u32) {
                        self.scheduler.as_ref().unwrap().schedule(
                            timeout,
                            RxTimeout,
                            *self.controller.as_ref().unwrap(),
                        );
                    }
                }
                Err(e) => {
                    // TODO: Notify self of starting read again?
                    log::error!("Error initiating DMA transfer: {:?}", e);
                    self.scheduler.as_ref().unwrap().schedule(
                        Milliseconds(10),
                        DmaRequest::RxStart,
                        *self.me.as_ref().unwrap(),
                    );
                }
            },
            Err(QueueError::BufferFull) => {
                // TODO: Go to sleep
                self.scheduler.as_ref().unwrap().schedule(
                    Milliseconds(10),
                    DmaRequest::RxStart,
                    *self.me.as_ref().unwrap(),
                );
            }
            Err(e) => {
                log::error!("Producer not ready, backing off: {:?}", e);
                self.scheduler.as_ref().unwrap().schedule(
                    Milliseconds(1000),
                    DmaRequest::RxStart,
                    *self.me.as_ref().unwrap(),
                );
            }
        }
    }
}

const READ_TIMEOUT: u32 = 100;
const READ_SIZE: usize = 255;

impl<U, T, TXN, RXN> Actor for UartInterrupt<U, T, TXN, RXN>
where
    U: DmaUartHal,
    T: Timer + 'static,
    TXN: ArrayLength<u8>,
    RXN: ArrayLength<u8>,
{
    type Configuration = (
        &'static U,
        Address<UartController<U>>,
        Address<T>,
        AsyncBBConsumer<TXN>,
        AsyncBBProducer<RXN>,
    );
    type Request = DmaRequest;
    type Response = ();

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.uart.replace(config.0);
        self.controller.replace(config.1);
        self.scheduler.replace(config.2);
        self.tx_consumer.replace(config.3);
        self.rx_producer.replace(config.4);
        self.me.replace(me);
    }

    fn on_start(mut self) -> Completion<Self> {
        let uart = self.uart.as_ref().unwrap();
        uart.enable_interrupt();
        self.start_read(READ_SIZE, Milliseconds(READ_TIMEOUT));
        self.start_write();
        Completion::immediate(self)
    }

    fn on_request(mut self, message: DmaRequest) -> Response<Self> {
        match message {
            DmaRequest::RxStart => self.start_read(READ_SIZE, Milliseconds(READ_TIMEOUT)),
            DmaRequest::TxStart => self.start_write(),
        }
        Response::immediate(self, ())
    }
}

pub enum DmaRequest {
    TxStart,
    RxStart,
}

impl<U, T, TXN, RXN> Interrupt for UartInterrupt<U, T, TXN, RXN>
where
    U: DmaUartHal,
    T: Timer + 'static,
    TXN: ArrayLength<u8>,
    RXN: ArrayLength<u8>,
{
    fn on_interrupt(&mut self) {
        let uart = self.uart.as_ref().unwrap();
        let (tx_done, rx_done) = uart.process_interrupts();
        log::trace!("[UART ISR] TX DONE: {}. RX DONE: {}", tx_done, rx_done,);

        if tx_done {
            let result = uart.finish_write();
            // log::info!("TX DONE: {:?}", result);
            if let Some(grant) = self.tx_consumer_grant.take() {
                let grant = grant.into_inner();
                if let Ok(_) = result {
                    let len = grant.len();
                    /*
                    log::info!("Releasing {} bytes from grant", len);*/
                    grant.release(len);
                } else {
                    grant.release(0);
                }
            }
        }

        if rx_done {
            let len = uart.finish_read();
            if let Some(grant) = self.rx_producer_grant.take() {
                if len > 0 {
                    let grant = grant.into_inner();
                    // log::info!("COMMITTING {} bytes", len);
                    grant.commit(len);
                }
            }
        }

        if tx_done {
            self.start_write();
        }

        if rx_done {
            self.start_read(READ_SIZE, Milliseconds(READ_TIMEOUT));
        }
    }
}


#[cfg(test)]
mod tests {
    /*
    extern crate std;
    use super::*;
    use crate::driver::timer::TimerActor;
    use core::sync::atomic::*;
    use futures::executor::block_on;
    use std::boxed::Box;

    struct TestTimer {}

    impl crate::hal::timer::Timer for TestTimer {
        fn start(&mut self, duration: Milliseconds) {}

        fn clear_update_interrupt_flag(&mut self) {}
    }

    struct TestHal {
        internal_buf: RefCell<[u8; 255]>,
        interrupt: Option<RefCell<UartInterrupt<Self, TimerActor<TestTimer>>>>,
        did_tx: AtomicBool,
        did_rx: AtomicBool,
    }

    impl TestHal {
        fn new() -> Self {
            Self {
                internal_buf: RefCell::new([0; 255]),
                interrupt: None,
                did_tx: AtomicBool::new(false),
                did_rx: AtomicBool::new(false),
            }
        }

        fn fire_interrupt(&self) {
            self.interrupt.as_ref().unwrap().borrow_mut().on_interrupt();
        }

        fn set_interrupt(&mut self, i: UartInterrupt<Self, TimerActor<TestTimer>>) {
            self.interrupt.replace(RefCell::new(i));
        }
    }

    impl DmaUartHal for TestHal {
        fn start_write(&self, tx_buffer: &[u8]) -> Result<(), Error> {
            {
                self.internal_buf.borrow_mut().copy_from_slice(tx_buffer);
                self.did_tx.store(true, Ordering::SeqCst);
            }
            self.fire_interrupt();
            Ok(())
        }

        fn finish_write(&self) -> Result<(), Error> {
            Ok(())
        }

        fn cancel_write(&self) {}

        fn prepare_read(&self, rx_buffer: &mut [u8]) -> Result<(), Error> {
            rx_buffer.copy_from_slice(&self.internal_buf.borrow()[..]);
            Ok(())
        }

        fn start_read(&self) {
            self.did_rx.store(true, Ordering::SeqCst);
            self.fire_interrupt();
        }

        fn finish_read(&self) -> Result<usize, Error> {
            if self.did_rx.load(Ordering::SeqCst) {
                Ok(self.internal_buf.borrow().len())
            } else {
                Ok(0)
            }
        }

        fn cancel_read(&self) {}

        fn process_interrupts(&self) -> (bool, bool) {
            (
                self.did_tx.swap(false, Ordering::SeqCst),
                self.did_rx.swap(false, Ordering::SeqCst),
            )
        }
    }

    struct TestIrq {}

    unsafe impl static_arena::interrupt::Nr for TestIrq {
        fn nr(&self) -> u8 {
            0
        }
    }
    */

    /*
    #[test]
    fn test_read() {
        let testuart = TestHal::new();
        let uart: DmaUart<TestHal, TimerActor<TestTimer>> = DmaUart::new(testuart, TestIrq {});
    }
    */
}

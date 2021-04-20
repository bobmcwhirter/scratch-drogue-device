extern crate embedded_hal;
use core::cell::RefCell;
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll};
use drogue_device_kernel::{
    actor::Actor, device::Device, device::DeviceContext, util::ImmediateFuture,
};
use embassy::traits::gpio::WaitForAnyEdge;
use embassy::util::Signal;
use embedded_hal::digital::v2::InputPin;
use std::vec::Vec;

/// A test context that can execute test for a given device
pub struct TestContext<D: Device + 'static> {
    runner: &'static TestRunner,
    device: DeviceContext<D>,
}

impl<D: Device> TestContext<D> {
    pub fn new(runner: &'static TestRunner, device: DeviceContext<D>) -> Self {
        Self { runner, device }
    }

    /// Configure context with a device
    pub fn configure(&mut self, device: D) {
        self.device.configure(device);
    }

    /// Create a test pin that can be used in tests
    pub fn pin(&mut self, initial: bool) -> TestPin {
        self.runner.pin(initial)
    }

    /// Create a signal that can be used in tests
    pub fn signal(&mut self) -> &'static TestSignal {
        self.runner.signal()
    }

    /// Mount the device, running the provided callback function.
    pub fn mount<F: FnOnce(&'static D) -> R, R>(&mut self, f: F) -> R {
        self.device.mount(f)
    }
}

impl<D: Device> Drop for TestContext<D> {
    fn drop(&mut self) {
        self.runner.done()
    }
}

/// A test context that can execute test for a given device
pub struct TestRunner {
    pins: UnsafeCell<Vec<InnerPin>>,
    signals: UnsafeCell<Vec<TestSignal>>,
    done: AtomicBool,
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            pins: UnsafeCell::new(Vec::new()),
            signals: UnsafeCell::new(Vec::new()),
            done: AtomicBool::new(false),
        }
    }

    /// Create a test pin that can be used in tests
    pub fn pin(&'static self, initial: bool) -> TestPin {
        let pins = unsafe { &mut *self.pins.get() };
        pins.push(InnerPin::new(initial));
        TestPin {
            inner: &pins[pins.len() - 1],
        }
    }

    /// Create a signal that can be used in tests
    pub fn signal(&'static self) -> &'static TestSignal {
        let signals = unsafe { &mut *self.signals.get() };
        signals.push(TestSignal::new());
        &signals[signals.len() - 1]
    }

    pub fn done(&'static self) {
        self.done.store(true, Ordering::SeqCst);
    }

    pub fn is_done(&'static self) -> bool {
        self.done.load(Ordering::SeqCst)
    }
}

/// A test message with an id that can be passed around to verify the system
#[derive(Copy, Clone)]
pub struct TestMessage(pub u32);

/// A test handler that carries a signal that is set on `on_message`
pub struct TestHandler {
    on_message: &'static TestSignal,
}

impl TestHandler {
    pub fn new(signal: &'static TestSignal) -> Self {
        Self { on_message: signal }
    }
}

impl Actor for TestHandler {
    type Configuration = ();
    type Message<'m> = TestMessage;
    type OnStartFuture<'m> = ImmediateFuture;
    type OnMessageFuture<'m> = ImmediateFuture;

    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        ImmediateFuture::new()
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        self.on_message.signal(*message);
        ImmediateFuture::new()
    }
}

/// A Pin that implements some embassy and embedded_hal traits that can be used to drive device changes.
pub struct TestPin {
    inner: &'static InnerPin,
}

struct InnerPin {
    value: AtomicBool,
    signal: Signal<()>,
}

impl Copy for TestPin {}
impl Clone for TestPin {
    fn clone(&self) -> Self {
        Self { inner: self.inner }
    }
}

impl TestPin {
    pub fn set_high(&self) {
        self.inner.set_value(true)
    }

    pub fn set_low(&self) {
        self.inner.set_value(false)
    }
}

impl InnerPin {
    pub fn new(initial: bool) -> Self {
        Self {
            value: AtomicBool::new(initial),
            signal: Signal::new(),
        }
    }

    fn set_value(&self, value: bool) {
        self.signal.reset();
        self.value.store(value, Ordering::SeqCst);
        self.signal.signal(());
    }

    fn get_value(&self) -> bool {
        self.value.load(Ordering::SeqCst)
    }

    fn wait_changed<'m>(&'m self) -> SignalFuture<'m> {
        SignalFuture {
            signal: &self.signal,
        }
    }
}

/// A future that awaits a signal
pub struct SignalFuture<'m> {
    signal: &'m Signal<()>,
}

impl<'m> Future for SignalFuture<'m> {
    type Output = ();
    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = self.signal.poll_wait(cx);
        result
    }
}

impl WaitForAnyEdge for TestPin {
    type Future<'m> = SignalFuture<'m>;
    fn wait_for_any_edge<'m>(&'m mut self) -> Self::Future<'m> {
        self.inner.wait_changed()
    }
}

impl InputPin for TestPin {
    type Error = ();
    fn is_high(&self) -> Result<bool, ()> {
        Ok(self.inner.get_value())
    }
    fn is_low(&self) -> Result<bool, ()> {
        Ok(!self.inner.get_value())
    }
}

/// A generic signal construct that can be used across actor and test states.
pub struct TestSignal {
    signal: Signal<()>,
    value: RefCell<Option<TestMessage>>,
}

impl TestSignal {
    pub fn new() -> Self {
        Self {
            signal: Signal::new(),
            value: RefCell::new(None),
        }
    }

    pub fn signal(&self, value: TestMessage) {
        self.value.borrow_mut().replace(value);
        self.signal.signal(())
    }

    pub fn message(&self) -> Option<TestMessage> {
        *self.value.borrow()
    }

    pub fn wait_signaled<'m>(&'m self) -> SignalFuture<'m> {
        SignalFuture {
            signal: &self.signal,
        }
    }
}

use crate::domain::time::duration::{Duration, Milliseconds};
use crate::prelude::{Actor, Address, Completion, NotifyHandler};

pub struct Schedule<A, DUR, E>
where
    A: Actor + NotifyHandler<E> + 'static,
    DUR: Duration + Into<Milliseconds>,
    E: 'static,
{
    pub delay: DUR,
    pub event: Option<E>,
    pub address: Address<A>,
}

impl<A, DUR, E> Schedule<A, DUR, E>
where
    A: Actor + NotifyHandler<E> + 'static,
    DUR: Duration + Into<Milliseconds>,
    E: 'static,
{
    pub fn new(delay: DUR, event: E, address: Address<A>) -> Self {
        Self {
            delay,
            event: Some(event),
            address,
        }
    }
}

pub trait Scheduler: Actor {
    fn schedule<A, DUR, E>(&mut self, schedule: Schedule<A, DUR, E>)
    where
        A: Actor + NotifyHandler<E> + 'static,
        DUR: Duration + Into<Milliseconds> + 'static,
        E: 'static;
}

impl<S, E, A, DUR> NotifyHandler<Schedule<A, DUR, E>> for S
where
    S: Scheduler + Actor + 'static,
    E: 'static,
    A: Actor + NotifyHandler<E> + 'static,
    DUR: Duration + Into<Milliseconds> + 'static,
{
    fn on_notify(mut self, message: Schedule<A, DUR, E>) -> Completion<Self> {
        self.schedule(message);
        Completion::immediate(self)
    }
}

impl<S: Scheduler> Address<S> {
    pub fn schedule<DUR, E, A>(&self, delay: DUR, event: E, address: Address<A>)
    where
        DUR: Duration + Into<Milliseconds> + 'static,
        E: 'static,
        A: Actor + NotifyHandler<E>,
    {
        self.notify(Schedule::new(delay, event, address));
    }
}

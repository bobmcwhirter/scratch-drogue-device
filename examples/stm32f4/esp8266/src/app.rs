use core::cell::RefCell;
use core::str::FromStr;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    traits::{ip::*, tcp::*, wifi::*},
    Actor,
};

pub enum Command {
    Send,
}

impl<D: WifiSupplicant + TcpStack> FromButtonEvent<Command> for App<D> {
    fn from(event: ButtonEvent) -> Option<Command> {
        match event {
            ButtonEvent::Pressed => None,
            ButtonEvent::Released => Some(Command::Send),
        }
    }
}

pub struct App<D: WifiSupplicant + TcpStack> {
    ssid: &'static str,
    psk: &'static str,
    ip: IpAddress,
    port: u16,
    driver: Option<D>,
    socket: Option<D::SocketHandle>,
}

impl<D: WifiSupplicant + TcpStack> App<D> {
    pub fn new(ssid: &'static str, psk: &'static str, ip: IpAddress, port: u16) -> Self {
        Self {
            ssid,
            psk,
            ip,
            port,
            socket: None,
            driver: None,
        }
    }
}

impl<D: WifiSupplicant + TcpStack> Unpin for App<D> {}

impl<D: WifiSupplicant + TcpStack> Actor for App<D> {
    type Configuration = D;
    #[rustfmt::skip]
    type Message<'m> where D: 'm = Command;
    #[rustfmt::skip]
    type OnStartFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.driver.replace(config);
    }

    fn on_start<'m>(mut self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {
            let mut driver = self.driver.take().unwrap();
            log::info!("Joining access point");
            let ip = driver
                .join(Join::Wpa {
                    ssid: heapless::String::from_str(self.ssid).unwrap(),
                    password: heapless::String::from_str(self.psk).unwrap(),
                })
                .await
                .expect("Error joining wifi");
            log::info!("Joined wifi network with IP: {}", ip);

            let socket = driver.open().await;

            log::info!("Connecting to {}:{}", self.ip, self.port);
            let result = driver
                .connect(
                    socket,
                    IpProtocol::Tcp,
                    SocketAddress::new(self.ip, self.port),
                )
                .await;
            match result {
                Ok(_) => {
                    self.driver.replace(driver);
                    self.socket.replace(socket);
                    log::info!("Connected to {:?}!", self.ip);
                }
                Err(e) => {
                    log::warn!("Error connecting: {:?}", e);
                }
            }
        }
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            match message {
                Command::Send => {
                    log::info!("Sending data..");

                    let mut driver = self.driver.take().expect("driver not bound!");
                    let socket = self.socket.take().expect("socket not bound!");
                    let result = driver.write(socket, b"{\"temp\": 24.3}\r\n").await;
                    match result {
                        Ok(_) => {
                            log::info!("Data sent");
                            let mut rx_buf = [0; 8];
                            loop {
                                let result = driver.read(socket, &mut rx_buf[..]).await;
                                match result {
                                    Ok(len) if &rx_buf[0..len] == b"OK\r\n" => {
                                        log::info!("Measurement confirmed");
                                        break;
                                    }
                                    Ok(len) if &rx_buf[0..len] == b"ERROR\r\n" => {
                                        log::info!("Error reporting measurement");
                                        break;
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        log::warn!("Error reading response: {:?}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Error sending measurement: {:?}", e);
                        }
                    }
                    self.driver.replace(driver);
                    self.socket.replace(socket);
                }
            }
        }
    }
}

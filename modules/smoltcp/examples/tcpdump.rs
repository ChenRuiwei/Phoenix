use std::{env, os::unix::io::AsRawFd};

use smoltcp::{
    phy::{wait as phy_wait, Device, RawSocket, RxToken},
    time::Instant,
    wire::{EthernetFrame, PrettyPrinter},
};

fn main() {
    let ifname = env::args().nth(1).unwrap();
    let mut socket = RawSocket::new(ifname.as_ref(), smoltcp::phy::Medium::Ethernet).unwrap();
    loop {
        phy_wait(socket.as_raw_fd(), None).unwrap();
        let (rx_token, _) = socket.receive(Instant::now()).unwrap();
        rx_token.consume(|buffer| {
            println!(
                "{}",
                PrettyPrinter::<EthernetFrame<&[u8]>>::new("", &buffer)
            );
        })
    }
}

use alloc::boxed::Box;

use async_trait::async_trait;
use config::mm::VIRT_RAM_OFFSET;
use sync::mutex::SpinNoIrqLock;
use uart_16550::MmioSerialPort;

use crate::CharDevice;

// TODO: implement async char device, can based on the crate, we check line
// status by ourselves

pub struct UartDevice {
    device: SpinNoIrqLock<uart_16550::MmioSerialPort>,
}

impl UartDevice {
    pub fn new() -> Self {
        const SERIAL_PORT_BASE_ADDRESS: usize = 0x1000_0000 + VIRT_RAM_OFFSET;

        let serial_port = unsafe { MmioSerialPort::new(SERIAL_PORT_BASE_ADDRESS) };

        Self {
            device: SpinNoIrqLock::new(serial_port),
        }
    }
}

#[async_trait]
impl CharDevice for UartDevice {
    async fn getchar(&self) -> u8 {
        self.device.lock().receive()
    }

    async fn puts(&self, chars: &[u8]) {
        for &c in chars {
            self.device.lock().send(c);
        }
    }

    fn poll_in(&self) -> bool {
        self.device.lock().poll_in()
    }

    fn poll_out(&self) -> bool {
        self.device.lock().poll_out()
    }

    fn handle_irq(&self) {
        todo!()
    }
}

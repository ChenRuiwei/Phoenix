use alloc::sync::Arc;

use sync::mutex::SpinNoIrqLock;
use uart_16550::MmioSerialPort;

use crate::CharDevice;

pub struct UartDevice {
    device: SpinNoIrqLock<uart_16550::MmioSerialPort>,
}

impl UartDevice {
    pub fn new() -> Self {
        const SERIAL_PORT_BASE_ADDRESS: usize = 0x1000_0000;

        let mut serial_port = unsafe { MmioSerialPort::new(SERIAL_PORT_BASE_ADDRESS) };

        Self {
            device: SpinNoIrqLock::new(serial_port),
        }
    }
}

impl CharDevice for UartDevice {
    fn getchar(&self) -> u8 {
        self.device.lock().receive()
    }

    fn puts(&self, chars: &[u8]) {
        for &c in chars {
            self.device.lock().send(c);
        }
    }

    fn handle_irq(&self) {
        todo!()
    }
}

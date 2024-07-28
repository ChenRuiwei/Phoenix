use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use device_core::{
    error::{DevError, DevResult},
    DeviceCapabilities, EthernetAddress, Medium, NetBufPtrOps, NetDevice,
};

/// The loopback interface operates at the network layer and handles the packets
/// directly at the IP level. Consequently, packets sent to 127.0.0.1 do not
/// include Ethernet headers because they never actually touch the physical
/// network hardware, which is necessary for Ethernet frame encapsulation
pub struct LoopbackDev {
    queue: VecDeque<Vec<u8>>,
}

impl LoopbackDev {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            queue: VecDeque::with_capacity(256),
        })
    }
}

impl NetDevice for LoopbackDev {
    #[inline]
    fn capabilities(&self) -> DeviceCapabilities {
        let mut cap = DeviceCapabilities::default();
        cap.max_transmission_unit = 65535;
        cap.max_burst_size = None;
        cap.medium = Medium::Ip;
        cap
    }

    fn mac_address(&self) -> EthernetAddress {
        EthernetAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    }

    fn can_transmit(&self) -> bool {
        true
    }

    fn can_receive(&self) -> bool {
        !self.queue.is_empty()
    }

    fn rx_queue_size(&self) -> usize {
        usize::MAX
    }

    fn tx_queue_size(&self) -> usize {
        usize::MAX
    }

    fn recycle_rx_buffer(&mut self, _rx_buf: Box<dyn NetBufPtrOps>) -> DevResult {
        Ok(())
    }

    fn recycle_tx_buffers(&mut self) -> DevResult {
        Ok(())
    }

    fn transmit(&mut self, tx_buf: Box<dyn NetBufPtrOps>) -> DevResult {
        let data = tx_buf.packet().to_vec();
        log::warn!("[NetDriverOps::transmit] now transmit {} bytes", data.len());
        self.queue.push_back(data);
        Ok(())
    }

    fn receive(&mut self) -> DevResult<Box<dyn NetBufPtrOps>> {
        if let Some(buf) = self.queue.pop_front() {
            log::warn!("[NetDriverOps::receive] now receive {} bytes", buf.len());
            Ok(Box::new(SimpleNetBuf(buf)))
        } else {
            Err(DevError::Again)
        }
    }

    fn alloc_tx_buffer(&mut self, size: usize) -> DevResult<Box<dyn NetBufPtrOps>> {
        let mut buffer = vec![0; size];
        buffer.resize(size, 0);
        Ok(Box::new(SimpleNetBuf(buffer)))
    }
}

struct SimpleNetBuf(Vec<u8>);

impl NetBufPtrOps for SimpleNetBuf {
    fn packet(&self) -> &[u8] {
        self.0.as_slice()
    }

    fn packet_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }

    fn packet_len(&self) -> usize {
        self.0.len()
    }
}

//! Copyright (c) 2023 MankorOS EastonMan
//!
//! driver for Synopsys DesignWare Mobile Storage Host Controller

use alloc::{boxed::Box, string::ToString, sync::Arc, vec::Vec};
use core::{cell::UnsafeCell, mem::size_of};

use byte_slice_cast::*;
use config::board::BLOCK_SIZE;
use device_core::{BlockDevice, DevId, Device, DeviceMajor, DeviceMeta, DeviceType};
use log::{debug, info, warn};
use memory::{alloc_frame_tracker, alloc_frame_trackers, FrameTracker, PhysAddr, VirtAddr};

use super::{
    dma::Descriptor,
    registers::{
        CtypeCardWidth, BLKSIZ, BMOD, BYTCNT, CDETECT, CID, CLKDIV, CLKENA, CMD, CMDARG, CTRL,
        CTYPE, DBADDRL, DBADDRU, IDSTS, PWREN, RESP, RINSTS, STATUS,
    },
};
use crate::wait_for;

#[derive(Debug)]
pub struct MMC {
    meta: DeviceMeta,
    fifo_offset: UnsafeCell<usize>,
    frames: UnsafeCell<Vec<FrameTracker>>,
}

unsafe impl Send for MMC {}
unsafe impl Sync for MMC {}

impl MMC {
    pub fn new(base_address: usize, size: usize, interrupt_number: usize) -> MMC {
        MMC {
            meta: DeviceMeta {
                dev_id: DevId {
                    major: DeviceMajor::Block,
                    minor: 1,
                },
                name: "snps,dw_mshc".to_string(),
                mmio_base: base_address,
                mmio_size: size,
                irq_no: None,
                dtype: DeviceType::Block,
            },
            fifo_offset: UnsafeCell::new(0x600),
            frames: UnsafeCell::new(Vec::new()),
        }
    }
    pub fn card_init(&self) {
        info!("====================== SDIO Init START ========================");

        info!("Card detect: {:b}", self.card_detect());
        info!("Power enable: {:b}", self.power_enable().power_enable());
        info!("Clock enable: {:b}", self.clock_enable().cclk_enable());
        info!("Card 0 width: {:?}", self.card_width(0));
        info!("Control register: {:?}", self.control_reg());
        info!("DMA enabled: {}", self.dma_enabled());
        info!(
            "Descriptor base address: {:x}",
            self.descriptor_base_address()
        );

        let card_idx = 0;
        // 0xAA is check pattern, see https://elixir.bootlin.com/linux/v6.4-rc7/source/drivers/mmc/core/sd_ops.c#L162
        const TEST_PATTERN: u32 = 0xAA;

        // Read clock divider
        info!("Read clock divider");
        let base = self.virt_base_address() as *mut CLKDIV;
        let clkdiv = unsafe { base.byte_add(CLKDIV::offset()).read_volatile() };
        info!("Clock divider: {:?}", clkdiv.clks());

        self.reset_clock();
        self.reset_fifo();
        self.set_controller_bus_width(card_idx, CtypeCardWidth::Width1);
        self.set_dma(true); // Disable DMA
        info!("Control register: {:?}", self.control_reg());

        let cmd = CMD::reset_cmd0(0);
        self.send_cmd(cmd, CMDARG::empty(), None, false);

        // SDIO Check
        // info!("SDIO Check");
        // // CMD5
        // let cmd = CMD::no_data_cmd(card_idx, 5);
        // let cmdarg = CMDARG::empty();
        // if self.send_cmd(cmd, cmdarg).is_none() {
        //     info!("No response from card, not SDIO");
        // }

        // Voltage check and SDHC 2.0 check
        info!("Voltage Check");
        // CMD8
        let cmd = CMD::no_data_cmd(card_idx, 8);
        let cmdarg = CMDARG::from((1 << 8) | TEST_PATTERN);
        let resp = self
            .send_cmd(cmd, cmdarg, None, false)
            .expect("Error sending command");
        if (resp.resp(0) & TEST_PATTERN) == 0 {
            warn!("Card {} unusable", card_idx);
        }

        // If card responses, consider it SD

        // Send ACMD41 to power up
        loop {
            // Send CMD55 before ACMD
            let cmd = CMD::no_data_cmd(card_idx, 55);
            let cmdarg = CMDARG::empty();
            self.send_cmd(cmd, cmdarg, None, false);
            let cmd = CMD::no_data_cmd_no_crc(card_idx, 41); // CRC is all 1 bit by design
            let cmdarg = CMDARG::from((1 << 30) | (1 << 24) | 0xFF8000);
            if let Some(resp) = self.send_cmd(cmd, cmdarg, None, false) {
                if resp.ocr() & (1 << 31) != 0 {
                    info!("Card {} powered up", card_idx);
                    if resp.ocr() & (1 << 30) != 0 {
                        info!("Card {} is high capacity", card_idx);
                    }
                    break;
                }
            }
            arch::spin(100000); // Wait for card to power up
        }

        // CMD2, get CID
        let cmd = CMD::no_data_cmd_no_crc(card_idx, 2).with_response_length(true); // R2 response, no CRC
        if let Some(resp) = self.send_cmd(cmd, CMDARG::empty(), None, false) {
            let cid = CID::from(resp.resps_u128());
            info!("CID: {:x?}", cid);
            info!(
                "Card Name: {}",
                core::str::from_utf8(cid.name().to_be_bytes().as_byte_slice()).unwrap()
            );
        }

        // CMD3, get RCA
        let cmd = CMD::no_data_cmd(card_idx, 3);
        let resp = self
            .send_cmd(cmd, CMDARG::empty(), None, false)
            .expect("Error executing CMD3");
        let rca = resp.resp(0) >> 16; // RCA[31:16]
        info!("Card status: {:x?}", resp.resp(0) & 0xFFFF);

        // CMD9, get CSD
        let cmd = CMD::no_data_cmd_no_crc(card_idx, 9).with_response_length(true); // R2 response, no CRC
        let cmdarg = CMDARG::from(rca << 16);
        self.send_cmd(cmd, cmdarg, None, false);

        // CMD7 select card
        let cmd = CMD::no_data_cmd(card_idx, 7);
        let cmdarg = CMDARG::from(rca << 16);
        let _resp = self
            .send_cmd(cmd, cmdarg, None, false)
            .expect("Error executing CMD7");

        info!("Current FIFO count: {}", self.fifo_filled_cnt());

        // ACMD51 get bus width
        // Send CMD55 before ACMD
        let cmd = CMD::no_data_cmd(card_idx, 55);
        let cmdarg = CMDARG::from(rca << 16);
        self.send_cmd(cmd, cmdarg, None, false); // RCA is required
        self.set_size(8, 8); // Set transfer size
        let cmd = CMD::data_cmd(card_idx, 51);
        let mut buffer: [usize; 64] = [0; 64]; // 512B
        self.send_cmd(cmd, CMDARG::empty(), Some(&mut buffer), true);
        info!("Current FIFO count: {}", self.fifo_filled_cnt());
        let resp = u64::from_be(self.read_fifo::<u64>());
        info!("Bus width supported: {:b}", (resp >> 48) & 0xF);

        // CMD16 set block length
        // let cmd = CMD::no_data_cmd(card_idx, 16);
        // let cmdarg = CMDARG::from(512);
        // self.send_cmd(cmd, cmdarg);

        info!("Current FIFO count: {}", self.fifo_filled_cnt());

        // Read one block
        self.set_size(512, 512);
        let cmd = CMD::data_cmd(card_idx, 17);
        let cmdarg = CMDARG::empty();
        let _resp = self
            .send_cmd(cmd, cmdarg, Some(&mut buffer), true)
            .expect("Error sending command");

        info!("Current FIFO count: {}", self.fifo_filled_cnt());

        let cmdarg = CMDARG::from(153);
        let _resp = self
            .send_cmd(cmd, cmdarg, Some(&mut buffer), true)
            .expect("Error sending command");
        debug!("Magic: 0x{:x}", buffer[0]);
        info!("Current FIFO count: {}", self.fifo_filled_cnt());

        // Try DMA

        // Allocate a page for descriptor table
        let frame = alloc_frame_tracker();
        let descriptor_page_paddr: PhysAddr = frame.ppn.to_paddr();
        unsafe { &mut *self.frames.get() }.push(frame);
        let descriptor_page_vaddr = descriptor_page_paddr.to_vaddr().bits();
        const DESCRIPTOR_CNT: usize = 2;
        let mut buffer_page_paddr: [usize; DESCRIPTOR_CNT] = [0; DESCRIPTOR_CNT];
        for i in 0..DESCRIPTOR_CNT {
            let frame = alloc_frame_tracker();
            buffer_page_paddr[i] = frame.ppn.to_paddr().bits();
            unsafe { &mut *self.frames.get() }.push(frame);
        }
        let _descriptor_table = unsafe {
            core::slice::from_raw_parts_mut(
                descriptor_page_vaddr as *mut Descriptor,
                DESCRIPTOR_CNT,
            )
        };

        // Build chain descriptor
        // for idx in 0..descriptor_cnt {
        //     descriptor_table[idx] = Descriptor::new(
        //         512,
        //         buffer_page_paddr[idx],
        //         descriptor_page_paddr + (idx + 1) % descriptor_cnt * 16, // 16B for
        // each     );
        // }
        // // Set descriptor base address
        // self.set_descript_base_address(descriptor_page_paddr);

        // // Enable DMA
        // self.set_dma(true);

        // // Read one block
        // let buffer = unsafe {
        //     core::slice::from_raw_parts_mut(
        //         kernel_phys_to_virt(buffer_page_paddr[0]) as *mut usize,
        //         64,
        //     )
        // };
        // debug!("Magic before: 0x{:x}", buffer[0]);
        // let cmdarg = CMDARG::from(0x200);
        // let resp = self.send_cmd(cmd, cmdarg, None).expect("Error sending command");

        // debug!("Magic after: 0x{:x}", buffer[0]);

        info!("Control register: {:?}", self.control_reg());
        let base = self.virt_base_address() as *mut u32;
        let rinsts: RINSTS = unsafe { base.byte_add(RINSTS::offset()).read_volatile() }.into();
        // Clear interrupt by writing 1
        unsafe {
            // Just clear all for now
            base.byte_add(RINSTS::offset())
                .write_volatile(rinsts.into());
        }
        info!("INT Status register: {:?}", rinsts);
        info!("======================= SDIO Init END ========================");
    }

    /// Internal function to send a command to the card
    fn send_cmd(
        &self,
        cmd: CMD,
        arg: CMDARG,
        buffer: Option<&mut [usize]>,
        is_read: bool,
    ) -> Option<RESP> {
        let base = self.virt_base_address() as *mut u32;

        // Sanity check
        if cmd.data_expected() && !self.dma_enabled() {
            debug_assert!(buffer.is_some())
        }

        let mut buffer_offset = 0;

        // Wait for can send cmd
        wait_for!({
            let cmd: CMD = unsafe { base.byte_add(CMD::offset()).read_volatile() }.into();
            cmd.can_send_cmd()
        });
        // Wait for card not busy if data is required
        if cmd.data_expected() {
            wait_for!({
                let status: STATUS =
                    unsafe { base.byte_add(STATUS::offset()).read_volatile() }.into();
                !status.data_busy()
            })
        }
        unsafe {
            // Set CMARG
            base.byte_add(CMDARG::offset()).write_volatile(arg.into());
            // Send CMD
            base.byte_add(CMD::offset()).write_volatile(cmd.into());
        }

        // Wait for cmd accepted
        wait_for!({
            let cmd: CMD = unsafe { base.byte_add(CMD::offset()).read_volatile() }.into();
            cmd.can_send_cmd()
        });

        // Wait for command done if response expected
        if cmd.response_expected() {
            wait_for!({
                let rinsts: RINSTS =
                    unsafe { base.byte_add(RINSTS::offset()).read_volatile() }.into();
                rinsts.command_done()
            });
        }

        // Wait for data transfer complete if data expected
        if cmd.data_expected() {
            let buffer = // TODO: dirty
                buffer.unwrap_or(unsafe { core::slice::from_raw_parts_mut(core::ptr::NonNull::dangling().as_ptr(), 64) });
            assert!(buffer_offset == 0);
            if is_read {
                wait_for!({
                    let rinsts: RINSTS =
                        unsafe { base.byte_add(RINSTS::offset()).read_volatile() }.into();
                    if rinsts.receive_data_request() && !self.dma_enabled() {
                        while self.fifo_filled_cnt() >= 2 {
                            buffer[buffer_offset] = self.read_fifo::<usize>();
                            buffer_offset += 1;
                        }
                    }
                    rinsts.data_transfer_over() || !rinsts.no_error()
                });
            } else {
                wait_for!({
                    let rinsts: RINSTS =
                        unsafe { base.byte_add(RINSTS::offset()).read_volatile() }.into();
                    if rinsts.transmit_data_request() && !self.dma_enabled() {
                        // Hard coded FIFO depth
                        while self.fifo_filled_cnt() < 120 {
                            buffer[buffer_offset] = self.read_fifo::<usize>();
                            buffer_offset += 1;
                        }
                    }
                    rinsts.data_transfer_over() || !rinsts.no_error()
                });
            }
            debug!("transmit {:?} bytes", (buffer_offset) * 8);
            debug!("Current oFIFO count: {}", self.fifo_filled_cnt());
            self.reset_fifo_offset();
        }

        // Check for error
        let rinsts: RINSTS = unsafe { base.byte_add(RINSTS::offset()).read_volatile() }.into();
        // Clear interrupt by writing 1
        unsafe {
            // Just clear all for now
            base.byte_add(RINSTS::offset())
                .write_volatile(rinsts.into());
        }

        // Check response
        let base = self.virt_base_address() as *mut RESP;
        let resp = unsafe { base.byte_add(RESP::offset()).read_volatile() };
        if rinsts.no_error() && !rinsts.command_conflict() {
            // No return for clock command
            if cmd.update_clock_register_only() {
                info!("Clock cmd done");
                return None;
            }
            debug!(
                "CMD{} done: {:?}, dma: {:?}",
                cmd.cmd_index(),
                rinsts.status(),
                self.dma_enabled()
            );
            Some(resp)
        } else {
            warn!("CMD{} error: {:?}", cmd.cmd_index(), rinsts.status());
            warn!("Dumping response");
            warn!("Response: {:x?}", resp);
            warn!("dma: {:?}", self.dma_enabled());
            None
        }
    }

    /// Read data from FIFO
    fn read_fifo<T>(&self) -> T {
        let base = self.virt_base_address() as *mut T;
        let result = unsafe { base.byte_add(*self.fifo_offset.get()).read_volatile() };
        unsafe { *self.fifo_offset.get() += size_of::<T>() };
        result
    }
    /// Write data to FIFO
    fn write_fifo<T>(&self, value: T) {
        let base = self.virt_base_address() as *mut T;
        unsafe {
            base.byte_add(*self.fifo_offset.get()).write_volatile(value);
            *self.fifo_offset.get() += size_of::<T>()
        };
    }
    /// Reset FIFO offset
    fn reset_fifo_offset(&self) {
        // Hard coded
        // From Synopsys documentation
        unsafe { *self.fifo_offset.get() = 0x600 };
    }

    /// Reset FIFO
    fn reset_fifo(&self) {
        let base = self.virt_base_address() as *mut CTRL;
        let ctrl = self.control_reg().with_fifo_reset(true);
        unsafe { base.byte_add(*self.fifo_offset.get()).write_volatile(ctrl) }
    }

    /// Set transaction size
    ///
    /// block_size: size of transfer
    /// byte_cnt: number of bytes to transfer
    fn set_size(&self, block_size: usize, byte_cnt: usize) {
        let blksiz = BLKSIZ::new().with_block_size(block_size);
        let bytcnt = BYTCNT::new().with_byte_count(byte_cnt);
        let base = self.virt_base_address() as *mut BLKSIZ;
        unsafe { base.byte_add(BLKSIZ::offset()).write_volatile(blksiz) };
        let base = self.virt_base_address() as *mut BYTCNT;
        unsafe { base.byte_add(BYTCNT::offset()).write_volatile(bytcnt) };
    }

    fn set_controller_bus_width(&self, card_index: usize, width: CtypeCardWidth) {
        let ctype = CTYPE::set_card_width(card_index, width);
        let base = self.virt_base_address() as *mut CTYPE;
        unsafe { base.byte_add(CTYPE::offset()).write_volatile(ctype) }
    }

    fn last_response_command_index(&self) -> usize {
        let base = self.virt_base_address() as *mut STATUS;
        let status = unsafe { base.byte_add(STATUS::offset()).read_volatile() };
        status.response_index()
    }

    fn fifo_filled_cnt(&self) -> usize {
        self.status().fifo_count()
    }
    fn status(&self) -> STATUS {
        let base = self.virt_base_address() as *mut STATUS;

        unsafe { base.byte_add(STATUS::offset()).read_volatile() }
    }

    fn card_detect(&self) -> usize {
        let base = self.virt_base_address() as *mut CDETECT;
        let cdetect = unsafe { base.byte_add(CDETECT::offset()).read_volatile() };
        !cdetect.card_detect_n() & 0xFFFF
    }

    fn power_enable(&self) -> PWREN {
        let base = self.virt_base_address() as *mut PWREN;

        unsafe { base.byte_add(PWREN::offset()).read_volatile() }
    }

    fn clock_enable(&self) -> CLKENA {
        let base = self.virt_base_address() as *mut CLKENA;

        unsafe { base.byte_add(CLKENA::offset()).read_volatile() }
    }

    fn set_dma(&self, enable: bool) {
        let base = self.virt_base_address() as *mut BMOD;
        let bmod = unsafe { base.byte_add(BMOD::offset()).read_volatile() };
        let bmod = bmod.with_idmac_enable(enable).with_software_reset(true);
        unsafe { base.byte_add(BMOD::offset()).write_volatile(bmod) };

        // Also reset the dma controller
        let base = self.virt_base_address() as *mut CTRL;
        let ctrl = unsafe { base.byte_add(CTRL::offset()).read_volatile() };
        let ctrl = ctrl.with_dma_reset(true).with_use_internal_dmac(enable);
        unsafe { base.byte_add(CTRL::offset()).write_volatile(ctrl) };
    }

    fn dma_enabled(&self) -> bool {
        let base = self.virt_base_address() as *mut BMOD;
        let bmod = unsafe { base.byte_add(BMOD::offset()).read_volatile() };
        bmod.idmac_enable()
    }

    fn dma_status(&self) -> IDSTS {
        let base = self.virt_base_address() as *mut IDSTS;

        unsafe { base.byte_add(IDSTS::offset()).read_volatile() }
    }

    fn card_width(&self, index: usize) -> CtypeCardWidth {
        let base = self.virt_base_address() as *mut CTYPE;
        let ctype = unsafe { base.byte_add(CTYPE::offset()).read_volatile() };
        ctype.card_width(index)
    }

    fn control_reg(&self) -> CTRL {
        let base = self.virt_base_address() as *mut CTRL;

        unsafe { base.byte_add(CTRL::offset()).read_volatile() }
    }

    fn descriptor_base_address(&self) -> usize {
        let base = self.virt_base_address() as *mut DBADDRL;
        let dbaddrl = unsafe { base.byte_add(DBADDRL::offset()).read_volatile() };
        let base = self.virt_base_address() as *mut DBADDRU;
        let dbaddru = unsafe { base.byte_add(DBADDRU::offset()).read_volatile() };
        dbaddru.addr() << 32 | dbaddrl.addr()
    }

    fn set_descript_base_address(&self, addr: usize) {
        let base = self.virt_base_address() as *mut u32;
        unsafe { base.byte_add(DBADDRL::offset()).write_volatile(addr as u32) };
        unsafe {
            base.byte_add(DBADDRU::offset())
                .write_volatile((addr >> 32) as u32)
        };
    }

    fn reset_clock(&self) {
        // Disable clock
        info!("Disable clock");
        let base = self.virt_base_address() as *mut CLKENA;
        let clkena = CLKENA::new().with_cclk_enable(0);
        unsafe { base.byte_add(CLKENA::offset()).write_volatile(clkena) };
        let cmd = CMD::clock_cmd();
        self.send_cmd(cmd, CMDARG::empty(), None, false);

        // Set clock divider
        info!("Set clock divider");
        let base = self.virt_base_address() as *mut CLKDIV;
        let clkdiv = CLKDIV::new().with_clk_divider0(4); // Magic, supposedly set to 400KHz
        unsafe { base.byte_add(CLKDIV::offset()).write_volatile(clkdiv) };

        // Re enable clock
        info!("Renable clock");
        let base = self.virt_base_address() as *mut CLKENA;
        let clkena = CLKENA::new().with_cclk_enable(1);
        unsafe { base.byte_add(CLKENA::offset()).write_volatile(clkena) };

        let cmd = CMD::clock_cmd();
        self.send_cmd(cmd, CMDARG::empty(), None, false);
    }
    fn virt_base_address(&self) -> usize {
        PhysAddr::from(self.mmio_base()).to_vaddr().bits()
    }
}

impl Device for MMC {
    fn meta(&self) -> &DeviceMeta {
        &self.meta
    }

    fn init(&self) {
        self.card_init()
    }

    fn handle_irq(&self) {
        todo!()
    }

    fn as_blk(self: Arc<Self>) -> Option<Arc<dyn BlockDevice>> {
        Some(self)
    }
}

impl BlockDevice for MMC {
    fn size(&self) -> u64 {
        16 * 1024 * 1024 * 1024
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn buffer_head_cnts(&self) -> usize {
        todo!()
    }

    fn remove_buffer_page(&self, block_id: usize) {
        todo!()
    }

    fn base_read_blocks(&self, block_id: usize, buf: &mut [u8]) {
        assert!(buf.len() == BLOCK_SIZE);
        let buf = unsafe { core::mem::transmute(buf) };
        debug!("reading block {}", block_id);
        // Read one block
        self.set_size(512, 512);
        let cmd = CMD::data_cmd(0, 17); // TODO: card number hard coded to 0
        let cmdarg = CMDARG::from(block_id as u32);
        self.send_cmd(cmd, cmdarg, Some(buf), true)
            .expect("Error sending command");
    }

    fn base_write_blocks(&self, block_id: usize, buf: &[u8]) {
        assert!(buf.len() == BLOCK_SIZE);
        #[allow(mutable_transmutes)]
        let buf = unsafe { core::mem::transmute(buf) };
        debug!("writing block {}", block_id);
        // Read one block
        self.set_size(512, 512);
        // CMD24 single block write
        let cmd = CMD::data_cmd(0, 24); // TODO: card number hard coded to 0
        let cmdarg = CMDARG::from(block_id as u32);
        self.send_cmd(cmd, cmdarg, Some(buf), false)
            .expect("Error sending command");
    }

    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.base_read_blocks(block_id, buf)
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.base_write_blocks(block_id, buf)
    }
}

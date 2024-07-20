use managed::ManagedSlice;

use super::Empty;
use crate::storage::{Full, RingBuffer};

/// Size and header of a packet.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PacketMetadata<H> {
    /// 数据包的大小
    size: usize,
    /// 数据包的头部信息
    header: Option<H>,
}

impl<H> PacketMetadata<H> {
    /// Empty packet description.
    pub const EMPTY: PacketMetadata<H> = PacketMetadata {
        size: 0,
        header: None,
    };

    /// 创建一个填充元数据
    fn padding(size: usize) -> PacketMetadata<H> {
        PacketMetadata { size, header: None }
    }

    /// 创建一个包含指定头部和大小的元数据
    fn packet(size: usize, header: H) -> PacketMetadata<H> {
        PacketMetadata {
            size,
            header: Some(header),
        }
    }

    /// 判断元数据是否为填充
    fn is_padding(&self) -> bool {
        self.header.is_none()
    }
}

/// An UDP packet ring buffer.
/// 用于管理UDP数据包的环形缓冲区
///
/// 例子：
/// ```
/// let mut metadata_storage = [PacketMetadata::EMPTY; 10]; // 可以存储10个数据包的元数据
/// let mut payload_storage = [0u8; 256]; // 可以存储256个字节的负载数据
/// let mut packet_buffer = PacketBuffer::new(&mut metadata_storage, &mut payload_storage);
/// ```
/// metadata_storage的大小是10，这意味着最多可以存储10个数据包的元数据。
/// 每个元数据包含数据包的大小和头部信息。
/// payload_storage的大小是256字节，
/// 这意味着所有数据包的负载（即数据部分）的总和不能超过256字节。
/// 如果10个数据包的负载总和超过了256字节，那么在存储新的数据包时会返回错误，
/// 因为缓冲区已满。

#[derive(Debug)]
pub struct PacketBuffer<'a, H: 'a> {
    /// 存储数据包元数据的环形缓冲区，包含一些关于数据包的描述性信息，
    /// 例如数据包的大小、数据包的头部信息（如源IP地址、目标IP地址、源端口、
    /// 目标端口等。PacketMetadata的数量与UDP数据报的数量一致
    metadata_ring: RingBuffer<'a, PacketMetadata<H>>,
    /// 存储数据包实际负载的环形缓冲区，是实际传输的数据内容，
    /// 比如在UDP数据包中，负载就是应用层传递的字节数据
    /// 所有的UDP数据报的负载长度之和不超过RingBuffer的长度
    payload_ring: RingBuffer<'a, u8>,
}

impl<'a, H> PacketBuffer<'a, H> {
    /// Create a new packet buffer with the provided metadata and payload
    /// storage.
    ///
    /// Metadata storage limits the maximum _number_ of packets in the buffer
    /// and payload storage limits the maximum _total size_ of packets.
    ///
    /// 创建一个新的PacketBuffer，使用提供的元数据存储和负载存储
    pub fn new<MS, PS>(metadata_storage: MS, payload_storage: PS) -> PacketBuffer<'a, H>
    where
        MS: Into<ManagedSlice<'a, PacketMetadata<H>>>,
        PS: Into<ManagedSlice<'a, u8>>,
    {
        PacketBuffer {
            metadata_ring: RingBuffer::new(metadata_storage),
            payload_ring: RingBuffer::new(payload_storage),
        }
    }

    /// Query whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.metadata_ring.is_empty()
    }

    /// Query whether the buffer is full.
    pub fn is_full(&self) -> bool {
        self.metadata_ring.is_full()
    }

    // There is currently no enqueue_with() because of the complexity of managing
    // padding in case of failure.

    /// Enqueue a single packet with the given header into the buffer, and
    /// return a reference to its payload, or return `Err(Full)`
    /// if the buffer is full.
    pub fn enqueue(&mut self, size: usize, header: H) -> Result<&mut [u8], Full> {
        // 如果缓冲区没有足够的空间，则返回Full错误
        if self.payload_ring.capacity() < size || self.metadata_ring.is_full() {
            return Err(Full);
        }

        // Ring is currently empty.  Clear it (resetting `read_at`) to maximize
        // for contiguous space.
        // 如果负载环缓冲区为空，重置它以优化连续空间
        if self.payload_ring.is_empty() {
            self.payload_ring.clear();
        }

        let window = self.payload_ring.window();
        let contig_window = self.payload_ring.contiguous_window();

        if window < size {
            return Err(Full);
        } else if contig_window < size {
            // 如果当前窗口不足以容纳新的数据包，但环形缓冲区可以通过填充绕回到起始位置，
            // 则进行填充。 注意是填充空数据，数据报负载不会被截断，
            // 也就是不会一半在结尾一半在开头
            if window - contig_window < size {
                // The buffer length is larger than the current contiguous window
                // and is larger than the contiguous window will be after adding
                // the padding necessary to circle around to the beginning of the
                // ring buffer.
                return Err(Full);
            } else {
                // Add padding to the end of the ring buffer so that the
                // contiguous window is at the beginning of the ring buffer.
                *self.metadata_ring.enqueue_one()? = PacketMetadata::padding(contig_window);
                // note(discard): function does not write to the result
                // enqueued padding buffer location
                let _buf_enqueued = self.payload_ring.enqueue_many(contig_window);
            }
        }
        // 将数据包元数据入队，然后将实际负载数据入队
        *self.metadata_ring.enqueue_one()? = PacketMetadata::packet(size, header);

        let payload_buf = self.payload_ring.enqueue_many(size);
        debug_assert!(payload_buf.len() == size);
        Ok(payload_buf)
    }

    /// Call `f` with a packet from the buffer large enough to fit `max_size`
    /// bytes. The packet is shrunk to the size returned from `f` and
    /// enqueued into the buffer.
    ///
    /// 类似于enqueue，但允许调用者传入一个闭包f来处理负载数据。
    /// 负载数据的大小由f返回
    pub fn enqueue_with_infallible<'b, F>(
        &'b mut self,
        max_size: usize,
        header: H,
        f: F,
    ) -> Result<usize, Full>
    where
        F: FnOnce(&'b mut [u8]) -> usize,
    {
        if self.payload_ring.capacity() < max_size || self.metadata_ring.is_full() {
            return Err(Full);
        }

        let window = self.payload_ring.window();
        let contig_window = self.payload_ring.contiguous_window();

        if window < max_size {
            return Err(Full);
        } else if contig_window < max_size {
            if window - contig_window < max_size {
                // The buffer length is larger than the current contiguous window
                // and is larger than the contiguous window will be after adding
                // the padding necessary to circle around to the beginning of the
                // ring buffer.
                return Err(Full);
            } else {
                // Add padding to the end of the ring buffer so that the
                // contiguous window is at the beginning of the ring buffer.
                *self.metadata_ring.enqueue_one()? = PacketMetadata::padding(contig_window);
                // note(discard): function does not write to the result
                // enqueued padding buffer location
                let _buf_enqueued = self.payload_ring.enqueue_many(contig_window);
            }
        }

        let (size, _) = self
            .payload_ring
            .enqueue_many_with(|data| (f(&mut data[..max_size]), ()));

        *self.metadata_ring.enqueue_one()? = PacketMetadata::packet(size, header);

        Ok(size)
    }

    /// 处理缓冲区中的填充数据，确保元数据和负载数据的一致性
    fn dequeue_padding(&mut self) {
        let _ = self.metadata_ring.dequeue_one_with(|metadata| {
            if metadata.is_padding() {
                // note(discard): function does not use value of dequeued padding bytes
                let _buf_dequeued = self.payload_ring.dequeue_many(metadata.size);
                Ok(()) // dequeue metadata
            } else {
                Err(()) // don't dequeue metadata
            }
        });
    }

    /// Call `f` with a single packet from the buffer, and dequeue the packet if
    /// `f` returns successfully, or return `Err(EmptyError)` if the buffer
    /// is empty.
    ///
    /// 从缓冲区中取出一个数据包，并调用给定的函数f处理该数据包。如果处理成功，
    /// 数据包将被出队列；如果缓冲区为空，则返回错误EmptyError
    /// - 'c：生命周期参数，表示缓冲区中数据的借用生命周期。
    /// - R：f函数成功时返回的结果类型。
    /// - E：f函数失败时返回的错误类型。
    /// - F：一个闭包类型，接受两个参数：
    ///   一个可变引用类型的H（元数据头）和一个可变引用的字节数组（数据包的负载），
    ///   返回Result<R, E>
    pub fn dequeue_with<'c, R, E, F>(&'c mut self, f: F) -> Result<Result<R, E>, Empty>
    where
        F: FnOnce(&mut H, &'c mut [u8]) -> Result<R, E>,
    {
        // 处理缓冲区填充
        self.dequeue_padding();
        // 从元数据环缓冲区中取出一个元数据，并调用传递的闭包处理这个元数据
        self.metadata_ring.dequeue_one_with(|metadata| {
            self.payload_ring
                .dequeue_many_with(|payload_buf| {
                    // 从负载环缓冲区中取出数据包，并确保其长度至少等于元数据中记录的大小
                    debug_assert!(payload_buf.len() >= metadata.size);

                    match f(
                        metadata.header.as_mut().unwrap(),
                        &mut payload_buf[..metadata.size],
                    ) {
                        // 如果闭包f成功处理数据包，返回元数据大小和处理结果Ok(val)
                        Ok(val) => (metadata.size, Ok(val)),
                        // 如果闭包f处理失败，返回大小0和错误结果Err(err)
                        Err(err) => (0, Err(err)),
                    }
                })
                .1
        })
    }

    /// Dequeue a single packet from the buffer, and return a reference to its
    /// payload as well as its header, or return `Err(Error::Exhausted)` if
    /// the buffer is empty.
    ///
    /// 从缓冲区中取出一个数据包，返回其头部和负载数据。如果缓冲区为空，
    /// 则返回Empty错误。
    pub fn dequeue(&mut self) -> Result<(H, &mut [u8]), Empty> {
        self.dequeue_padding();

        let meta = self.metadata_ring.dequeue_one()?;

        let payload_buf = self.payload_ring.dequeue_many(meta.size);
        debug_assert!(payload_buf.len() == meta.size);
        Ok((meta.header.take().unwrap(), payload_buf))
    }

    /// Peek at a single packet from the buffer without removing it, and return
    /// a reference to its payload as well as its header, or return
    /// `Err(Error:Exhausted)` if the buffer is empty.
    ///
    /// This function otherwise behaves identically to
    /// [dequeue](#method.dequeue).
    ///
    /// 查看缓冲区中的第一个数据包，而不将其移除。返回其头部和负载数据。
    /// 如果缓冲区为空，则返回Empty错误。
    pub fn peek(&mut self) -> Result<(&H, &[u8]), Empty> {
        self.dequeue_padding();

        if let Some(metadata) = self.metadata_ring.get_allocated(0, 1).first() {
            Ok((
                metadata.header.as_ref().unwrap(),
                self.payload_ring.get_allocated(0, metadata.size),
            ))
        } else {
            Err(Empty)
        }
    }

    /// Return the maximum number packets that can be stored.
    /// 返回缓冲区中可以存储的最大数据包数量
    pub fn packet_capacity(&self) -> usize {
        self.metadata_ring.capacity()
    }

    /// Return the maximum number of bytes in the payload ring buffer.
    /// 返回负载环缓冲区的最大字节数。
    pub fn payload_capacity(&self) -> usize {
        self.payload_ring.capacity()
    }

    /// Reset the packet buffer and clear any staged.
    /// 重置缓冲区，清除所有已存储的数据包
    #[allow(unused)]
    pub(crate) fn reset(&mut self) {
        self.payload_ring.clear();
        self.metadata_ring.clear();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn buffer() -> PacketBuffer<'static, ()> {
        // 四个UDP数据报，他们的负载长度之和要小于16
        PacketBuffer::new(vec![PacketMetadata::EMPTY; 4], vec![0u8; 16])
    }

    #[test]
    fn test_simple() {
        let mut buffer = buffer();
        buffer.enqueue(6, ()).unwrap().copy_from_slice(b"abcdef");
        assert_eq!(buffer.enqueue(16, ()), Err(Full));
        assert_eq!(buffer.metadata_ring.len(), 1);
        assert_eq!(buffer.dequeue().unwrap().1, &b"abcdef"[..]);
        assert_eq!(buffer.dequeue(), Err(Empty));
    }

    #[test]
    fn test_peek() {
        let mut buffer = buffer();
        assert_eq!(buffer.peek(), Err(Empty));
        buffer.enqueue(6, ()).unwrap().copy_from_slice(b"abcdef");
        assert_eq!(buffer.metadata_ring.len(), 1);
        assert_eq!(buffer.peek().unwrap().1, &b"abcdef"[..]);
        assert_eq!(buffer.dequeue().unwrap().1, &b"abcdef"[..]);
        assert_eq!(buffer.peek(), Err(Empty));
    }

    #[test]
    fn test_padding() {
        let mut buffer = buffer();
        assert!(buffer.enqueue(6, ()).is_ok());
        assert!(buffer.enqueue(8, ()).is_ok());
        assert!(buffer.dequeue().is_ok());
        buffer.enqueue(4, ()).unwrap().copy_from_slice(b"abcd");
        assert_eq!(buffer.metadata_ring.len(), 3);
        assert!(buffer.dequeue().is_ok());

        assert_eq!(buffer.dequeue().unwrap().1, &b"abcd"[..]);
        assert_eq!(buffer.metadata_ring.len(), 0);
    }

    #[test]
    fn test_padding_with_large_payload() {
        let mut buffer = buffer();
        assert!(buffer.enqueue(12, ()).is_ok());
        assert!(buffer.dequeue().is_ok());
        buffer
            .enqueue(12, ())
            .unwrap()
            .copy_from_slice(b"abcdefghijkl");
    }

    #[test]
    fn test_dequeue_with() {
        let mut buffer = buffer();
        assert!(buffer.enqueue(6, ()).is_ok());
        assert!(buffer.enqueue(8, ()).is_ok());
        assert!(buffer.dequeue().is_ok());
        buffer.enqueue(4, ()).unwrap().copy_from_slice(b"abcd");
        assert_eq!(buffer.metadata_ring.len(), 3);
        assert!(buffer.dequeue().is_ok());

        assert!(matches!(
            buffer.dequeue_with(|_, _| Result::<(), u32>::Err(123)),
            Ok(Err(_))
        ));
        assert_eq!(buffer.metadata_ring.len(), 1);

        assert!(buffer
            .dequeue_with(|&mut (), payload| {
                assert_eq!(payload, &b"abcd"[..]);
                Result::<(), ()>::Ok(())
            })
            .is_ok());
        assert_eq!(buffer.metadata_ring.len(), 0);
    }

    #[test]
    fn test_metadata_full_empty() {
        let mut buffer = buffer();
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
        assert!(buffer.enqueue(1, ()).is_ok());
        assert!(!buffer.is_empty());
        assert!(buffer.enqueue(1, ()).is_ok());
        assert!(buffer.enqueue(1, ()).is_ok());
        assert!(!buffer.is_full());
        assert!(!buffer.is_empty());
        assert!(buffer.enqueue(1, ()).is_ok());
        assert!(buffer.is_full());
        assert!(!buffer.is_empty());
        assert_eq!(buffer.metadata_ring.len(), 4);
        assert_eq!(buffer.enqueue(1, ()), Err(Full));
    }

    #[test]
    fn test_window_too_small() {
        let mut buffer = buffer();
        assert!(buffer.enqueue(4, ()).is_ok());
        assert!(buffer.enqueue(8, ()).is_ok());
        assert!(buffer.dequeue().is_ok());
        assert_eq!(buffer.enqueue(16, ()), Err(Full));
        assert_eq!(buffer.metadata_ring.len(), 1);
    }

    #[test]
    fn test_contiguous_window_too_small() {
        let mut buffer = buffer();
        assert!(buffer.enqueue(4, ()).is_ok());
        assert!(buffer.enqueue(8, ()).is_ok());
        assert!(buffer.dequeue().is_ok());
        assert_eq!(buffer.enqueue(8, ()), Err(Full));
        assert_eq!(buffer.metadata_ring.len(), 1);
    }

    #[test]
    fn test_contiguous_window_wrap() {
        let mut buffer = buffer();
        assert!(buffer.enqueue(15, ()).is_ok());
        assert!(buffer.dequeue().is_ok());
        assert!(buffer.enqueue(16, ()).is_ok());
    }

    #[test]
    fn test_capacity_too_small() {
        let mut buffer = buffer();
        assert_eq!(buffer.enqueue(32, ()), Err(Full));
    }

    #[test]
    fn test_contig_window_prioritized() {
        let mut buffer = buffer();
        assert!(buffer.enqueue(4, ()).is_ok());
        assert!(buffer.dequeue().is_ok());
        assert!(buffer.enqueue(5, ()).is_ok());
    }

    #[test]
    fn clear() {
        let mut buffer = buffer();

        // Ensure enqueuing data in teh buffer fills it somewhat.
        assert!(buffer.is_empty());
        assert!(buffer.enqueue(6, ()).is_ok());

        // Ensure that resetting the buffer causes it to be empty.
        assert!(!buffer.is_empty());
        buffer.reset();
        assert!(buffer.is_empty());
    }
}

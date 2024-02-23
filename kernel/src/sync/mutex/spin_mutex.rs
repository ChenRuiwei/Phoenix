use crate::arch::interrupts;

/// A spin-based lock providing mutually exclusive access to data.
pub struct Mutex<T: ?Sized> {
    inner: spin::Mutex<T>,
}

impl<T> Mutex<T> {
    /// Creates a new [`Mutex`] wrapping the supplied data.
    pub const fn new(value: T) -> Self {
        Self {
            inner: spin::Mutex::new(value),
        }
    }

    /// Locks the [`Mutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned value may be dereferenced for data access and the lock will be dropped
    /// when the guard falls out of scope.
    pub fn lock(&self) -> MutexGuard<T> {
        MutexGuard {
            guard: core::mem::ManuallyDrop::new(self.inner.lock()),
            irq_lock: false,
        }
    }

    /// Locks the [`Mutex`] and returns a IRQ guard that permits access to the inner data and
    /// disables interrupts while the lock is held.
    ///
    /// The returned value may be dereferenced for data access and the lock will be dropped and
    /// interrupts will be re-enabled when the guard falls out of scope. Deadlocks occur if a thread
    /// tries to acquire a lock that will never become free. Thus, locking interrupts is useful for
    /// volatile operations where we might be interrupted.
    pub fn lock_irq(&self) -> MutexGuard<T> {
        let irq_lock = interrupts::is_enabled();

        unsafe {
            interrupts::disable();
        }

        MutexGuard {
            guard: core::mem::ManuallyDrop::new(self.inner.lock()),
            irq_lock,
        }
    }

    /// Force unlock this [`Mutex`].
    ///
    /// # Safety
    ///
    /// This is *extremely* unsafe if the lock is not held by the current thread. However, this
    /// can be useful in some instances for exposing the lock to FFI that doesn't know how to deal
    /// with RAII.
    pub unsafe fn force_unlock(&self) {
        self.inner.force_unlock()
    }
}

pub struct MutexGuard<'a, T: ?Sized + 'a> {
    guard: core::mem::ManuallyDrop<spin::MutexGuard<'a, T>>,
    irq_lock: bool,
}

impl<'a, T: ?Sized> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.guard.deref()
    }
}

impl<'a, T: ?Sized> core::ops::DerefMut for MutexGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.guard.deref_mut()
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            core::mem::ManuallyDrop::drop(&mut self.guard);
        }

        if self.irq_lock {
            unsafe {
                interrupts::enable();
            }
        }
    }
}

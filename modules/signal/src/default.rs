use crate::sigset::Sig;

pub trait DefaultSignalHandler {
    fn ignore(sig: Sig) {
        log::debug!("ignore this sig {}", sig);
    }
    fn terminate(sig: Sig) {
        log::info!("terminate this sig {}", sig);
    }
    fn stop(sig: Sig) {
        log::info!("stop this sig {}", sig);
    }
    fn core(sig: Sig) {
        log::info!("core this sig {}", sig);
    }
}

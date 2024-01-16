use thiserror::Error;

#[allow(clippy::upper_case_acronyms)]
#[derive(Error, Debug)]
pub enum HypervisorError {
    #[error("Util error")]
    Util,
    #[error("KvmIoctl")]
    KvmIoctl {
        #[from]
        source: kvm_ioctls::Error,
    },
}

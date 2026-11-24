use super::{Code, lock};
use std::{
    sync::{Mutex, atomic::AtomicBool},
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, watch},
    time::timeout,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Cleanup {
    Pending,
    Reaped,
    Unconfirmed,
}

pub(super) struct Job {
    pub(super) cancel: AtomicBool,
    cleanup: watch::Sender<Cleanup>,
    permit: Mutex<Option<OwnedSemaphorePermit>>,
}

impl Job {
    pub(super) fn new(permit: OwnedSemaphorePermit) -> Self {
        let (cleanup, _) = watch::channel(Cleanup::Pending);
        Self {
            cancel: AtomicBool::new(false),
            cleanup,
            permit: Mutex::new(Some(permit)),
        }
    }

    pub(super) fn finish(&self, status: Cleanup) {
        if status == Cleanup::Reaped {
            lock(&self.permit).take();
        }
        self.cleanup.send_replace(status);
    }

    pub(super) async fn wait_for_cleanup(&self, duration: Duration) -> Result<(), Code> {
        let mut receiver = self.cleanup.subscribe();
        let result = timeout(
            duration,
            receiver.wait_for(|state| *state != Cleanup::Pending),
        )
        .await;
        match result {
            Ok(Ok(state)) if *state == Cleanup::Reaped => Ok(()),
            _ => Err(Code::DriverError),
        }
    }
}

/// A blocking worker panic or dropped task cannot serve as proof of reaping.
pub(super) struct Completion<'a> {
    job: &'a Job,
    reported: bool,
}

impl<'a> Completion<'a> {
    pub(super) fn new(job: &'a Job) -> Self {
        Self {
            job,
            reported: false,
        }
    }

    pub(super) fn report(mut self, status: Cleanup) {
        self.job.finish(status);
        self.reported = true;
    }
}

impl Drop for Completion<'_> {
    fn drop(&mut self) {
        if !self.reported {
            self.job.finish(Cleanup::Unconfirmed);
        }
    }
}

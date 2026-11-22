use super::*;

fn test_job() -> (Arc<Semaphore>, Arc<Job>) {
    let lanes = Arc::new(Semaphore::new(1));
    let permit = lanes.clone().try_acquire_owned().unwrap();
    (lanes, Arc::new(Job::new(permit)))
}

#[tokio::test]
async fn safety_ack_requires_cleanup_and_keeps_the_run_slot_reserved() {
    let (lanes, job) = test_job();
    let deadline = Duration::from_millis(20);
    assert_eq!(job.wait_for_cleanup(deadline).await, Err(Code::DriverError));
    assert_eq!(lanes.available_permits(), 0);

    job.finish(Cleanup::Reaped);
    assert_eq!(job.wait_for_cleanup(deadline).await, Ok(()));
    assert_eq!(lanes.available_permits(), 1);
}

#[tokio::test]
async fn unconfirmed_worker_keeps_the_slot_and_rejects_safe_ack() {
    let (lanes, job) = test_job();
    job.finish(Cleanup::Unconfirmed);
    for _ in 0..2 {
        assert_eq!(
            job.wait_for_cleanup(Duration::from_secs(1)).await,
            Err(Code::DriverError)
        );
        assert_eq!(lanes.available_permits(), 0);
    }
}

#[test]
fn abandoned_start_cancels_the_worker_and_fences_its_result() {
    let (_, job) = test_job();
    let session = Arc::new(Session::default());
    {
        let mut state = lock(&session.state);
        state.status = State::Running;
        state.generation = 7;
        state.owner = Some("e1".into());
        state.job = Some(job.clone());
    }

    drop(StartWait::new(session.clone(), job.clone(), 7));

    let state = lock(&session.state);
    assert_eq!(state.status, State::Fault);
    assert_eq!(state.owner.as_deref(), Some("e1"));
    assert!(!state.current_job(7, &job));
    assert!(Arc::ptr_eq(state.job.as_ref().unwrap(), &job));
    assert!(state.capture.is_none());
    assert!(job.cancel.load(Ordering::Acquire));
}

#[test]
fn unavailable_physical_station_cannot_claim_safe_or_energize() {
    let target = pb::Target {
        station_id: PHYSICAL_STATION.into(),
        experiment_id: "e1".into(),
    };
    let safe = physical_operation(&target, Operation::EnterSafeState).unwrap();
    assert!(!safe.accepted);
    assert_eq!(safe.state, "OFFLINE");
    assert_eq!(safe.result["hardware_applied"], false);
    for operation in [
        Operation::Start,
        Operation::ApplyRouting,
        Operation::GuardedPowerUp,
    ] {
        assert_eq!(
            physical_operation(&target, operation).unwrap_err(),
            Code::HardwareUnavailable
        );
    }
}

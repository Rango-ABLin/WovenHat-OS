#[path = "../kernel/src/update.rs"]
mod update;

use update::{BootDecision, Error, SlotState, UpdateState};

#[test]
fn candidate_activation_is_a_bounded_ab_transaction() {
    let mut state = UpdateState::new(10, 128, [1; 32]).unwrap();
    let slot = state.stage_candidate(11, 256, [2; 32]).unwrap();
    assert_eq!(slot, 1);
    state.activate_candidate(slot).unwrap();
    assert_eq!(state.active_slot(), 0);
    assert_eq!(state.pending_slot(), Some(slot));
    assert_eq!(state.boot_target(), Ok(BootDecision::Attempt { slot, attempt: 1 }));
    state.boot_succeeded(slot).unwrap();
    assert_eq!(state.active_slot(), 1);
    assert_eq!(state.slot(0).unwrap().state, SlotState::Ready);
    assert_eq!(state.slot(1).unwrap().state, SlotState::Active);
}

#[test]
fn repeated_failed_boots_roll_back_and_reject_generation_reuse() {
    let mut state = UpdateState::new(1, 1, [1; 32]).unwrap();
    let slot = state.stage_candidate(2, 1, [2; 32]).unwrap();
    state.activate_candidate(slot).unwrap();
    for _ in 0..3 {
        assert!(matches!(state.boot_target(), Ok(BootDecision::Attempt { .. })));
        state.boot_failed(slot).unwrap();
    }
    assert_eq!(state.boot_target(), Ok(BootDecision::RolledBack(0)));
    assert_eq!(state.slot(slot).unwrap().state, SlotState::Bad);
    assert_eq!(state.stage_candidate(1, 1, [3; 32]), Err(Error::GenerationRollback));
}

#[test]
fn malformed_candidates_never_enter_a_slot() {
    let mut state = UpdateState::new(1, 1, [1; 32]).unwrap();
    assert_eq!(state.stage_candidate(2, 0, [2; 32]), Err(Error::InvalidImage));
    assert_eq!(state.stage_candidate(2, 1, [0; 32]), Err(Error::InvalidImage));
    assert_eq!(state.slot(1).unwrap().state, SlotState::Empty);
}

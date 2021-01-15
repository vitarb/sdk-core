use rustfsm::{fsm, TransitionResult};

fsm! {
    name SideEffectMachine; command SideEffectCommand; error SideEffectMachineError;

    Created --(Schedule, on_schedule) --> MarkerCommandCreated;
    Created --(Schedule, on_schedule) --> MarkerCommandCreatedReplaying;

    MarkerCommandCreated --(CommandRecordMarker, on_command_record_marker) --> ResultNotified;

    MarkerCommandCreatedReplaying --(CommandRecordMarker) --> ResultNotifiedReplaying;

    ResultNotified --(MarkerRecorded, on_marker_recorded) --> MarkerCommandRecorded;

    ResultNotifiedReplaying --(MarkerRecorded, on_marker_recorded) --> MarkerCommandRecorded;
}

#[derive(thiserror::Error, Debug)]
pub enum SideEffectMachineError {}

pub enum SideEffectCommand {}

#[derive(Default, Clone)]
pub struct Created {}

impl Created {
    pub fn on_schedule(self) -> SideEffectMachineTransition {
        unimplemented!()
    }
}

#[derive(Default, Clone)]
pub struct MarkerCommandCreated {}

impl MarkerCommandCreated {
    pub fn on_command_record_marker(self) -> SideEffectMachineTransition {
        unimplemented!()
    }
}

#[derive(Default, Clone)]
pub struct MarkerCommandCreatedReplaying {}

#[derive(Default, Clone)]
pub struct MarkerCommandRecorded {}

#[derive(Default, Clone)]
pub struct ResultNotified {}

impl ResultNotified {
    pub fn on_marker_recorded(self) -> SideEffectMachineTransition {
        unimplemented!()
    }
}

#[derive(Default, Clone)]
pub struct ResultNotifiedReplaying {}

impl ResultNotifiedReplaying {
    pub fn on_marker_recorded(self) -> SideEffectMachineTransition {
        unimplemented!()
    }
}

impl From<MarkerCommandCreatedReplaying> for ResultNotifiedReplaying {
    fn from(_: MarkerCommandCreatedReplaying) -> Self {
        Self::default()
    }
}
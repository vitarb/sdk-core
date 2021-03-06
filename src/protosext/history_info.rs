use crate::{
    machines::{WFMachinesError, WorkflowMachines},
    protos::temporal::api::enums::v1::EventType,
    protos::temporal::api::history::v1::{History, HistoryEvent},
};

#[derive(Clone, Debug, derive_more::Constructor, PartialEq)]
pub(crate) struct HistoryInfo {
    pub previous_started_event_id: i64,
    pub workflow_task_started_event_id: i64,
    pub events: Vec<HistoryEvent>,
}

type Result<T, E = HistoryInfoError> = std::result::Result<T, E>;

#[derive(thiserror::Error, Debug)]
pub enum HistoryInfoError {
    #[error("Latest wf started id and previous one are equal! ${previous_started_event_id:?}")]
    UnexpectedEventId {
        previous_started_event_id: i64,
        workflow_task_started_event_id: i64,
    },
    #[error("Invalid history! Event {0:?} should be WF task completed, failed, or timed out")]
    FailedOrTimeout(HistoryEvent),
    #[error("Last item in history wasn't WorkflowTaskStarted")]
    HistoryEndsUnexpectedly,
    #[error("Underlying error in workflow machine")]
    UnderlyingMachineError(#[from] WFMachinesError),
}

impl HistoryInfo {
    /// Constructs a new instance, retaining only enough events to reach the provided workflow
    /// task number. If not provided, all events are retained.
    pub(crate) fn new_from_events(
        events: &[HistoryEvent],
        to_wf_task_num: Option<usize>,
    ) -> Result<Self> {
        let to_wf_task_num = to_wf_task_num.unwrap_or(usize::MAX);
        let mut workflow_task_started_event_id = 0;
        let mut previous_started_event_id = 0;
        let mut count = 0;
        let mut history = events.iter().peekable();
        let mut events = vec![];

        while let Some(event) = history.next() {
            events.push(event.clone());
            let next_event = history.peek();

            if event.event_type == EventType::WorkflowTaskStarted as i32 {
                let next_is_completed = next_event.map_or(false, |ne| {
                    ne.event_type == EventType::WorkflowTaskCompleted as i32
                });
                let next_is_failed_or_timeout = next_event.map_or(false, |ne| {
                    ne.event_type == EventType::WorkflowTaskFailed as i32
                        || ne.event_type == EventType::WorkflowTaskTimedOut as i32
                });

                if next_event.is_none() || next_is_completed {
                    previous_started_event_id = workflow_task_started_event_id;
                    workflow_task_started_event_id = event.event_id;
                    if workflow_task_started_event_id == previous_started_event_id {
                        return Err(HistoryInfoError::UnexpectedEventId {
                            previous_started_event_id,
                            workflow_task_started_event_id,
                        });
                    }
                    count += 1;
                    if count == to_wf_task_num || next_event.is_none() {
                        return Ok(Self {
                            previous_started_event_id,
                            workflow_task_started_event_id,
                            events,
                        });
                    }
                } else if next_event.is_some() && !next_is_failed_or_timeout {
                    return Err(HistoryInfoError::FailedOrTimeout(event.clone()));
                }
            }

            if next_event.is_none() {
                if event.is_final_wf_execution_event() {
                    return Ok(Self {
                        previous_started_event_id,
                        workflow_task_started_event_id,
                        events,
                    });
                }
                // No more events
                if workflow_task_started_event_id != event.event_id {
                    return Err(HistoryInfoError::HistoryEndsUnexpectedly);
                }
            }
        }
        unreachable!()
    }

    pub(crate) fn new_from_history(h: &History, to_wf_task_num: Option<usize>) -> Result<Self> {
        Self::new_from_events(&h.events, to_wf_task_num)
    }

    /// Apply events from history to workflow machines. Remember that only the events that exist
    /// in this instance will be applied, which is determined by `to_wf_task_num` passed into the
    /// constructor.
    pub(crate) fn apply_history_events(&self, wf_machines: &mut WorkflowMachines) -> Result<()> {
        let (_, events) = self
            .events
            .split_at(wf_machines.get_last_started_event_id() as usize);
        let mut history = events.iter().peekable();

        wf_machines.set_started_ids(
            self.previous_started_event_id,
            self.workflow_task_started_event_id,
        );
        let mut started_id = self.previous_started_event_id;

        while let Some(event) = history.next() {
            let next_event = history.peek();

            if event.event_type == EventType::WorkflowTaskStarted as i32 {
                let next_is_completed = next_event.map_or(false, |ne| {
                    ne.event_type == EventType::WorkflowTaskCompleted as i32
                });
                let next_is_failed_or_timeout = next_event.map_or(false, |ne| {
                    ne.event_type == EventType::WorkflowTaskFailed as i32
                        || ne.event_type == EventType::WorkflowTaskTimedOut as i32
                });

                if next_event.is_none() || next_is_completed {
                    started_id = event.event_id;
                    if next_event.is_none() {
                        wf_machines.handle_event(event, false)?;
                        return Ok(());
                    }
                } else if next_event.is_some() && !next_is_failed_or_timeout {
                    return Err(HistoryInfoError::FailedOrTimeout(event.clone()));
                }
            }

            wf_machines.handle_event(event, next_event.is_some())?;

            if next_event.is_none() {
                if event.is_final_wf_execution_event() {
                    return Ok(());
                }
                if started_id != event.event_id {
                    return Err(HistoryInfoError::UnexpectedEventId {
                        previous_started_event_id: started_id,
                        workflow_task_started_event_id: event.event_id,
                    });
                }
                unreachable!()
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        machines::test_help::TestHistoryBuilder,
        protos::temporal::api::history::v1::{history_event, TimerFiredEventAttributes},
    };

    #[test]
    fn history_info_constructs_properly() {
        /*
            1: EVENT_TYPE_WORKFLOW_EXECUTION_STARTED
            2: EVENT_TYPE_WORKFLOW_TASK_SCHEDULED
            3: EVENT_TYPE_WORKFLOW_TASK_STARTED
            4: EVENT_TYPE_WORKFLOW_TASK_COMPLETED
            5: EVENT_TYPE_TIMER_STARTED
            6: EVENT_TYPE_TIMER_FIRED
            7: EVENT_TYPE_WORKFLOW_TASK_SCHEDULED
            8: EVENT_TYPE_WORKFLOW_TASK_STARTED
        */
        let mut t = TestHistoryBuilder::default();

        t.add_by_type(EventType::WorkflowExecutionStarted);
        t.add_workflow_task();
        let timer_started_event_id = t.add_get_event_id(EventType::TimerStarted, None);
        t.add(
            EventType::TimerFired,
            history_event::Attributes::TimerFiredEventAttributes(TimerFiredEventAttributes {
                started_event_id: timer_started_event_id,
                timer_id: "timer1".to_string(),
            }),
        );
        t.add_workflow_task_scheduled_and_started();
        let history_info = t.get_history_info(1).unwrap();
        assert_eq!(3, history_info.events.len());
        let history_info = t.get_history_info(2).unwrap();
        assert_eq!(8, history_info.events.len());
    }
}

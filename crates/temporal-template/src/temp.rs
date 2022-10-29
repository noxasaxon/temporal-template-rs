use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash, str::FromStr};
use strum::{Display, EnumDiscriminants, EnumIter, EnumString, IntoEnumIterator};

pub const SLACK_INFO_DELIMITER: &str = ",";
pub const TEMPORAL_KEY_DELIMITER: &str = ":";
pub const USER_INFO_DELIMITER: &str = "~";

#[derive(EnumIter, EnumString, Display, PartialEq, Eq, Hash, Debug)]
pub enum KeysToTemporalAction {
    /// Temporal Event Type (signal, query, execute)
    E,
    /// Workflow_id
    W,
    /// Namespace
    N,
    /// Taskqueue
    T,
    /// workflow tYpe aka fn name
    Y,
    /// workflow Run_id
    R,
    /// Signal name
    S,
    /// Query type
    Q,
    /// qUery args
    U,
}

impl KeysToTemporalAction {
    pub fn to_kv(&self, value: &str) -> String {
        format!("{}{}{}", self, TEMPORAL_KEY_DELIMITER, value)
    }

    pub fn get_value<'a>(&self, encoder_map: &mut HashMap<Self, &'a str>) -> Result<&'a str> {
        encoder_map.remove(self).ok_or_else(|| {
            anyhow!(
                "temporal key: `{:?}` not supplied in callback_id. encoder_map =  {:?}",
                self,
                encoder_map
            )
        })
    }
}

pub fn build_slack_info_string(temporal_interaction: TemporalInteraction) -> String {
    let mut kv_pairs = Vec::new();

    let namespace = temporal_interaction.namespace();
    let task_queue = temporal_interaction.task_queue();
    let workflow_id = temporal_interaction.workflow_id();

    // set event type from outer enum variant
    kv_pairs.push(KeysToTemporalAction::E.to_kv(&temporal_interaction.to_type_string()));

    match temporal_interaction {
        TemporalInteraction::Execute(action) => {
            for key in KeysToTemporalAction::iter() {
                kv_pairs.push(key.to_kv(match key {
                    KeysToTemporalAction::W => &workflow_id,
                    KeysToTemporalAction::N => &namespace,
                    KeysToTemporalAction::T => &task_queue,
                    KeysToTemporalAction::Y => &action.workflow_type,
                    _ => continue,
                }))
            }
        }
        TemporalInteraction::Signal(action) => {
            for key in KeysToTemporalAction::iter() {
                kv_pairs.push(match key {
                    KeysToTemporalAction::W => key.to_kv(&workflow_id),
                    KeysToTemporalAction::N => key.to_kv(&namespace),
                    KeysToTemporalAction::T => key.to_kv(&task_queue),
                    KeysToTemporalAction::R => key.to_kv(&action.run_id()),
                    KeysToTemporalAction::S => key.to_kv(&action.signal_name),
                    _ => continue,
                })
            }
        }
        TemporalInteraction::Query(action) => {
            for key in KeysToTemporalAction::iter() {
                match key {
                    KeysToTemporalAction::W => key.to_kv(&workflow_id),
                    KeysToTemporalAction::N => key.to_kv(&namespace),
                    KeysToTemporalAction::T => key.to_kv(&task_queue),
                    KeysToTemporalAction::Q => key.to_kv(&action.query_type),
                    KeysToTemporalAction::U => key.to_kv(&action.query_args()),
                    _ => continue,
                };
            }
        }
    }

    kv_pairs.join(",")
}

#[derive(Serialize, Deserialize, Debug, PartialEq, EnumDiscriminants)]
#[serde(tag = "type")]
#[strum_discriminants(derive(EnumString, Display))]
pub enum TemporalInteraction {
    Execute(ExecuteTemporalWorkflow),
    Signal(SignalTemporal),
    Query(QueryTemporal),
}

impl TemporalInteraction {
    pub fn to_type_string(&self) -> String {
        match self {
            TemporalInteraction::Execute(_) => {
                TemporalInteractionDiscriminants::Execute.to_string()
            }
            TemporalInteraction::Signal(_) => TemporalInteractionDiscriminants::Signal.to_string(),
            TemporalInteraction::Query(_) => TemporalInteractionDiscriminants::Query.to_string(),
        }
    }

    pub fn to_slack_string(self) -> String {
        build_slack_info_string(self)
    }

    pub fn workflow_id(&self) -> String {
        match self {
            TemporalInteraction::Execute(action) => action.workflow_id.clone(),
            TemporalInteraction::Signal(action) => action
                .workflow_id
                .as_ref()
                .map_or("".into(), |some| some.clone()),
            TemporalInteraction::Query(action) => action
                .workflow_id
                .as_ref()
                .map_or("".into(), |some| some.clone()),
        }
    }

    pub fn task_queue(&self) -> String {
        match self {
            TemporalInteraction::Execute(action) => action.task_queue.clone(),
            TemporalInteraction::Signal(action) => action.task_queue.clone(),
            TemporalInteraction::Query(action) => action.task_queue.clone(),
        }
    }

    pub fn namespace(&self) -> String {
        match self {
            TemporalInteraction::Execute(action) => action.namespace.clone(),
            TemporalInteraction::Signal(action) => action.namespace.clone(),
            TemporalInteraction::Query(action) => action.namespace.clone(),
        }
    }

    pub fn add_data_args(self, args: Option<Vec<serde_json::Value>>) -> Self {
        match self {
            Self::Execute(exec) => Self::Execute(ExecuteTemporalWorkflow { args, ..exec }),
            Self::Signal(signal) => Self::Signal(SignalTemporal {
                input: args,
                ..signal
            }),
            Self::Query(query) => Self::Query(QueryTemporal {
                query_args: args,
                ..query
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct ExecuteTemporalWorkflow {
    pub namespace: String,
    pub task_queue: String,
    pub workflow_id: String,
    /// the Workflow's Function name
    pub workflow_type: String,
    pub args: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct SignalTemporal {
    pub namespace: String,
    pub task_queue: String,
    pub workflow_id: Option<String>,
    pub run_id: Option<String>,
    pub signal_name: String,
    pub input: Option<Vec<serde_json::Value>>,
    pub identity: Option<String>,
    pub request_id: Option<String>,
    pub control: Option<String>,
}

impl SignalTemporal {
    pub fn run_id(&self) -> String {
        self.run_id.as_ref().map_or("".into(), |some| some.clone())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
pub struct TemporalWorkflowExecutionInfo {
    pub workflow_id: String,
    pub run_id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct QueryTemporal {
    pub namespace: String,
    pub task_queue: String,
    pub workflow_id: Option<String>,
    pub run_id: Option<String>,
    pub query_type: String,
    pub query_args: Option<Vec<serde_json::Value>>,
}

impl QueryTemporal {
    pub fn run_id(&self) -> String {
        self.run_id.as_ref().map_or("".into(), |some| some.clone())
    }

    pub fn query_args(&self) -> String {
        self.query_args.as_ref().map_or("".into(), |some| {
            some.first()
                .map_or_else(|| "".to_string(), |arg| arg.to_string())
        })
    }
}

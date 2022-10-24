use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use temporal_client::{self, ConfiguredClient, RetryClient, TemporalServiceClientWithMetrics};
use temporal_sdk_core_protos::{
    coresdk::AsJsonPayloadExt,
    temporal::api::{
        common::v1::{Payload, Payloads, WorkflowExecution, WorkflowType},
        enums::v1::TaskQueueKind,
        query::v1::WorkflowQuery,
        taskqueue::v1::TaskQueue,
        workflowservice::v1::{
            QueryWorkflowRequest, QueryWorkflowResponse, SignalWorkflowExecutionRequest,
            SignalWorkflowExecutionResponse, StartWorkflowExecutionRequest,
            StartWorkflowExecutionResponse,
        },
    },
};
use toolbox::{get_host_from_env, get_port_from_env};
use uuid::Uuid;

pub const DEFAULT_TEMPORAL_ROLE: &str = "TEMPORAL";
pub const DEFAULT_NAMESPACE: &str = "security-engineering";

pub type TemporalSDKClient = RetryClient<ConfiguredClient<TemporalServiceClientWithMetrics>>;

pub async fn build_temporal_client_without_namespace(role: &str) -> Result<TemporalSDKClient> {
    let temporal_url = url::Url::parse(&format!(
        "http://{}:{}",
        get_host_from_env(role)?,
        get_port_from_env(role)?
    ))?;

    let client_options = temporal_client::ClientOptionsBuilder::default()
        .identity("seceng_rust_apig".into())
        .client_name("")
        .client_version("")
        .target_url(temporal_url.clone())
        .build()
        .unwrap();

    client_options
        .connect_no_namespace(None, None)
        .await
        .with_context(|| format!("Failed to create Temporal Client at url {temporal_url}"))
}

pub async fn query_temporal(query_info: QueryTemporal) -> Result<QueryWorkflowResponse> {
    let mut client = build_temporal_client_without_namespace(DEFAULT_TEMPORAL_ROLE).await?;

    let query_response = client
        .get_client_mut()
        .workflow_svc_mut()
        .query_workflow(QueryWorkflowRequest {
            namespace: query_info.namespace,
            execution: Some(WorkflowExecution {
                workflow_id: todo!(),
                run_id: todo!(),
            }),
            query: Some(WorkflowQuery {
                query_type: todo!(),
                query_args: todo!(),
                header: todo!(),
            }),
            query_reject_condition: todo!(),
        })
        .await?;

    Ok(query_response.into_inner())
}

pub async fn signal_temporal(
    signal_info: SignalTemporal,
) -> Result<SignalWorkflowExecutionResponse> {
    let input = signal_info.input.map(|inputs| Payloads {
        payloads: inputs
            .into_iter()
            .map(|arg| arg.as_json_payload().unwrap())
            .collect(),
    });

    let mut client = build_temporal_client_without_namespace(DEFAULT_TEMPORAL_ROLE).await?;

    let signal_response = client
        .get_client_mut()
        .workflow_svc_mut()
        .signal_workflow_execution(SignalWorkflowExecutionRequest {
            namespace: signal_info.namespace,
            workflow_execution: Some(WorkflowExecution {
                workflow_id: todo!(),
                run_id: todo!(),
            }),
            signal_name: signal_info.signal_name,
            input,
            identity: signal_info.identity,
            request_id: signal_info.request_id,
            control: signal_info.control,
            header: None,
        })
        .await?;

    Ok(signal_response.into_inner())
}

pub async fn start_temporal_workflow(
    workflow_info: ExecuteTemporalWorkflow,
) -> Result<StartWorkflowExecutionResponse> {
    let mut client = build_temporal_client_without_namespace(DEFAULT_TEMPORAL_ROLE).await?;

    let workflow_execution_request = build_workflow_execution_request(
        workflow_info.namespace,
        workflow_info.args,
        workflow_info.task_queue,
        workflow_info.workflow_id,
        workflow_info.workflow_type,
        None,
    );

    let execution_response = client
        .get_client_mut()
        .workflow_svc_mut()
        .start_workflow_execution(workflow_execution_request)
        .await?;

    Ok(execution_response.into_inner())
}

pub fn to_json_payloads(args: Vec<serde_json::Value>) -> Payloads {
    Payloads {
        payloads: args
            .iter()
            .map(|arg| arg.as_json_payload().unwrap())
            .collect(),
    }
}

pub fn build_workflow_execution_request(
    namespace: String,
    input: Option<Vec<serde_json::Value>>,
    task_queue: String,
    workflow_id: String,
    workflow_type: String,
    options: Option<temporal_client::WorkflowOptions>,
) -> StartWorkflowExecutionRequest {
    let options = options.unwrap_or_default();

    let input = input.map(|wf_input_args| to_json_payloads(wf_input_args));

    StartWorkflowExecutionRequest {
        namespace,
        input,
        workflow_id,
        workflow_type: Some(WorkflowType {
            name: workflow_type,
        }),
        task_queue: Some(TaskQueue {
            name: task_queue,
            kind: TaskQueueKind::Unspecified as i32,
        }),
        request_id: Uuid::new_v4().to_string(),
        workflow_id_reuse_policy: options.id_reuse_policy as i32,
        workflow_execution_timeout: options.execution_timeout.and_then(|d| d.try_into().ok()),
        workflow_run_timeout: options.execution_timeout.and_then(|d| d.try_into().ok()),
        workflow_task_timeout: options.task_timeout.and_then(|d| d.try_into().ok()),
        search_attributes: options.search_attributes.and_then(|d| d.try_into().ok()),
        cron_schedule: options.cron_schedule.unwrap_or_default(),
        ..Default::default()
    }
}

/// Data Models ///////////////////////////////////////////////////

// {
//     "type" : "ExecuteWorkflow",
//     "namespace" : "security-engineering",
//     "task_queue": "template-taskqueue",
//     "workflow_id" : "1",
//     "workflow_type" : "GreetingWorkflow",
//     "args":[{
//         "name" : "saxon",
//         "team" : "seceng"
//     }]
// }

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum TemporalInteraction {
    ExecuteWorkflow(ExecuteTemporalWorkflow),
    Signal(SignalTemporal),
    // Query(QueryTemporal),
}

impl TemporalInteraction {
    pub async fn execute(self) -> Result<TemporalInteractionResponse> {
        let response = match self {
            TemporalInteraction::ExecuteWorkflow(wf_info) => {
                TemporalInteractionResponse::new_from_exec_response(
                    start_temporal_workflow(wf_info).await?,
                )
            }
            TemporalInteraction::Signal(signal_info) => {
                TemporalInteractionResponse::new_from_signal_response(
                    signal_temporal(signal_info).await?,
                )
            } // TemporalInteraction::Query(query_info) => query_temporal(query_info).await,
        };

        Ok(response)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ExecuteTemporalWorkflow {
    pub namespace: String,
    pub task_queue: String,
    pub workflow_id: String,
    /// the Workflow's Function name
    pub workflow_type: String,
    pub args: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SignalTemporal {
    pub namespace: String,
    pub task_queue: String,
    workflow_execution: Option<TemporalWorkflowExecutionInfo>,
    signal_name: String,
    input: Option<Vec<serde_json::Value>>,
    identity: String,
    request_id: String,
    control: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TemporalWorkflowExecutionInfo {
    workflow_id: String,
    run_id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct QueryTemporal {
    pub namespace: String,
    pub task_queue: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum TemporalInteractionResponse {
    ExecuteWorkflow(TemporalExecuteWorkflowResponse),
    Signal(TemporalSignalResponse),
    // Query(TemporalQueryResponse),
}

impl TemporalInteractionResponse {
    pub fn new_from_exec_response(exec_response: StartWorkflowExecutionResponse) -> Self {
        Self::ExecuteWorkflow(TemporalExecuteWorkflowResponse {
            run_id: exec_response.run_id,
        })
    }

    pub fn new_from_signal_response(signal_response: SignalWorkflowExecutionResponse) -> Self {
        Self::Signal(TemporalSignalResponse {})
        // Self::Signal(TemporalSignalResponse {run_id: signal_response. })
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TemporalExecuteWorkflowResponse {
    run_id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TemporalSignalResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TemporalQueryResponse {}

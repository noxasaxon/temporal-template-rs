mod temp;

use anyhow::Result;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use slack_morphism::prelude::*;
use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use temporal_sdk::{
    sdk_client_options, ActContext, ActivityOptions, WfContext, WfExitValue, Worker,
};
use temporal_sdk_core::{
    init_worker, protos::coresdk::AsJsonPayloadExt, telemetry_init, TelemetryOptionsBuilder, Url,
};
use temporal_sdk_core_api::worker::WorkerConfigBuilder;
use temporal_sdk_core_protos::coresdk::activity_result::activity_resolution::Status;

use crate::temp::{build_slack_info_string, SignalTemporal, TemporalInteraction};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("starting test worker server");

    let server_options = sdk_client_options(Url::from_str("http://localhost:7233")?).build()?;

    let client = server_options.connect("default", None, None).await?;

    let telemetry_options = TelemetryOptionsBuilder::default().build()?;
    telemetry_init(&telemetry_options)?;

    let worker_config = WorkerConfigBuilder::default()
        .namespace("security-engineering")
        .task_queue("task_queue")
        .worker_build_id("some_unique_thing")
        .build()?;

    let core_worker = init_worker(worker_config, client);

    let mut worker = Worker::new_from_core(Arc::new(core_worker), "task_queue");
    worker.register_activity(
        "echo_activity",
        |_ctx: ActContext, echo_me: String| async move { Ok(echo_me) },
    );

    worker.register_activity("test_activity_fn", test_activity_fn);

    worker.register_activity("test_slack_activity", test_slack_activity);

    // testing new stuff for workflow functions
    worker.register_wf("test_workflow_fn", test_workflow_fn);

    worker.register_wf(
        "test_workflow_fn_that_waits_for_signal",
        test_workflow_fn_that_waits_for_signal,
    );

    worker.run().await?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TestActInput {
    name: String,
    team: String,
}

async fn test_activity_fn(ctx: ActContext, input: TestActInput) -> Result<String> {
    println!("{:?} - Activity time before waiting", Instant::now());
    tokio::time::sleep(Duration::from_secs(5)).await;
    println!("{:?} - Activity time AFTER waiting", Instant::now());

    let msg = format!(
        "Hello {}, from team {}",
        input.name,
        input.team.to_uppercase()
    );

    println!("from activity: {}", &msg);
    Ok(msg)
}

#[derive(Serialize, Deserialize)]
struct TestWFInput {
    name: String,
    team: String,
}

/// Current core_sdk won't let you return anything from WF
// async fn test_workflow_fn(input: TestWFInput) -> Result<String> {
async fn test_workflow_fn(ctx: WfContext) -> Result<WfExitValue<()>> {
    // workflow inputs need to be manually deserialized into their actual type(s)
    let args = ctx.get_args();
    let input: TestWFInput =
        serde_json::from_slice(&args.first().expect("No argument passed to workflow").data)
            .expect("Failed to deserialize wf arg into expected input struct");

    // testing log from workflow
    let msg = format!(
        "Hello {}, from team {}",
        input.name,
        input.team.to_uppercase()
    );

    // testing time from workflow
    println!("{:?} - Workflow time before Activity", Instant::now());

    // wait for activity to finish. activity sleeps for 5 seconds and writes some logs, returning a string
    let resp = ctx
        .activity(ActivityOptions {
            activity_type: "test_activity_fn".to_string(),
            start_to_close_timeout: Some(Duration::from_secs(50)),
            // activity fn can only take a single argument
            input: input.as_json_payload().expect("serializes fine"),
            ..Default::default()
        })
        .await;

    println!("{:?} - Workflow time after Activity", Instant::now());

    println!("activity resp debug: {:?}", &resp);

    let activity_output_bytes = match resp.status {
        Some(finished) => match finished {
            Status::Completed(success) => success.result.expect("no result returned").data,
            _ => todo!(),
        },
        _ => todo!(),
    };

    println!(
        "activity resp data: {}",
        String::from_utf8(activity_output_bytes).expect("Activity didn't return a string Type")
    );

    println!("from workflow: {}", &msg);

    // Ok(WfExitValue::Normal(()))
    Ok(().into())
}

const SIGNAL_NAME: &str = "test-signal";

/// Current core_sdk won't let you return anything from WF
// async fn test_workflow_fn(input: TestWFInput) -> Result<String> {
async fn test_workflow_fn_that_waits_for_signal(ctx: WfContext) -> Result<WfExitValue<()>> {
    println!("{:?} - Workflow time before activity", Instant::now());
    let resp = ctx
        .activity(ActivityOptions {
            activity_type: "test_slack_activity".to_string(),
            start_to_close_timeout: Some(Duration::from_secs(50)),
            // activity fn can only take a single argument
            input: "some_name".as_json_payload().expect("serializes fine"),
            ..Default::default()
        })
        .await;
    println!("{:?} - Workflow time AFTER activity", Instant::now());

    println!("{:?} - Workflow time before waiting signal", Instant::now());
    let signal_resp = ctx.make_signal_channel(SIGNAL_NAME).next().await.unwrap();
    println!("{:?} - Workflow time AFTER waiting signal", Instant::now());

    let json_values = signal_resp.input[..]
        .iter()
        .map(|pload| serde_json::to_value(&pload.data).unwrap())
        .collect::<Vec<serde_json::Value>>();

    println!("{:?} - signal resp", json_values);

    println!(" - End workflow");

    // Ok(WfExitValue::Normal(()))
    Ok(().into())
}

async fn test_slack_activity(ctx: ActContext, channel_id: String) -> Result<()> {
    println!("{:?} - Activity time before slack", Instant::now());

    let client = SlackClient::new(SlackClientHyperConnector::new());

    let slack_xoxb_token = std::env::var("SLACK_TOKEN")?;

    let token: SlackApiToken = SlackApiToken::new(slack_xoxb_token.into());

    // Sessions are lightweight and basically just a reference to client and token
    let session = client.open_session(&token);
    println!("{:#?}", session);

    let activity_info = ctx.get_info();
    let execution = activity_info.workflow_execution.as_ref().unwrap();

    let signal = TemporalInteraction::Signal(SignalTemporal {
        namespace: activity_info.workflow_namespace.clone(),
        task_queue: activity_info.task_queue.clone(),
        workflow_id: Some(execution.workflow_id.clone()),
        run_id: Some(execution.run_id.clone()),
        signal_name: SIGNAL_NAME.to_string(),
        ..Default::default()
    });

    let parsed_temporal_info_with_action_id = build_slack_info_string(signal).into();

    let test = session
        .chat_post_message(&slack_morphism::api::SlackApiChatPostMessageRequest::new(
            channel_id.into(),
            SlackMessageContent::new()
                .with_text("hi".into())
                .with_blocks(slack_blocks![
                    some_into(SlackSectionBlock::new().with_text(md!("hi "))),
                    some_into(SlackDividerBlock::new()),
                    some_into(SlackHeaderBlock::new(pt!("Simple header"))),
                    some_into(SlackActionsBlock::new(slack_blocks![some_into(
                        SlackBlockButtonElement::new(
                            "simple-message-button".into(),
                            pt!("Simple button text")
                        )
                        .with_action_id(parsed_temporal_info_with_action_id)
                    )]))
                ]),
        ))
        .await?;

    println!("{:#?}", test);

    println!("{:?} - Activity time AFTER slack", Instant::now());

    Ok(())
}

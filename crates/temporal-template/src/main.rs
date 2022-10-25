use anyhow::Result;
use serde::{Deserialize, Serialize};
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

    // testing new stuff for workflow functions
    worker.register_wf("test_workflow_fn", test_workflow_fn);

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

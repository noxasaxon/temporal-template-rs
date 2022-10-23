use temporal_sdk_helpers::activity_server_example_main;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("starting test worker server");

    activity_server_example_main().await
}

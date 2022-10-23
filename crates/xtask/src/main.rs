/// https://github.com/matklad/cargo-xtask/
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
};

type DynError = Box<dyn std::error::Error>;

// commands
const CMD_SOMETHING: &str = "do-something";
const CMD_PARALLEL_WORK: &str = "do-it-parallel";

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{e}");
        std::process::exit(-1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some(c) if c == CMD_SOMETHING => do_cmd("asdf")?,
        Some(c) if c == CMD_PARALLEL_WORK => {
            let thread_handles = ["cmd1", "cmd2"].map(|cmd_name| {
                thread::spawn(|| do_cmd(cmd_name).expect("failed to do command : {cmd_name}"))
            });

            let mut errors = Vec::new();
            for t in thread_handles {
                if let Err(msg) = t.join() {
                    errors.push(msg);
                }
            }

            if !errors.is_empty() {
                return Err(format!("{:?}", errors).into());
            }
        }
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "\nTasks:
{CMD_SOMETHING}                 do a thing
{CMD_PARALLEL_WORK}             do a thing in parallel
"
    )
}

#[allow(dead_code)]
fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        // .nth(1)  would be `./crates`, .nth(2) is the Root
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn do_cmd(test: &str) -> Result<(), String> {
    Ok(())
}

// fn build_lambda(package_name: &str) -> Result<(), DynError> {
//     let status = Command::new("docker")
//         .current_dir(project_root())
//         .args(&[
//             "run",
//             "--rm",
//             "-t",
//             "-v",
//             &format!("{}:/home/rust/src", project_root().display()),
//             "messense/rust-musl-cross:aarch64-musl",
//             "cargo",
//             "build",
//             &format!("--package={package_name}"),
//             "--release",
//         ])
//         .status()?;

//     if !status.success() {
//         return Err("cargo build failed".into());
//     }
//     Ok(())
// }

// fn copy_lambda_binary_to_terraform_dir(package_name: &str) -> Result<(), DynError> {
//     let binary_path = project_root().join(format!(
//         "target/aarch64-unknown-linux-musl/release/{package_name}"
//     ));

//     let destination_dir =
//         project_root().join(format!("terraform_aws/serverless/archives/{package_name}"));

//     fs::create_dir_all(&destination_dir)?;
//     fs::copy(&binary_path, destination_dir.join("bootstrap"))?;

//     Ok(())
// }

// fn prep_lambda_for_terraform(package_name: &str) -> Result<(), DynError> {
//     build_lambda(package_name)?;
//     copy_lambda_binary_to_terraform_dir(package_name)?;
//     Ok(())
// }

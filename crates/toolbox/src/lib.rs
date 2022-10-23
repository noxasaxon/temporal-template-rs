use anyhow::{Context, Result};

pub fn get_env_var(env_var_name: &str) -> Result<String> {
    std::env::var(env_var_name)
        .with_context(|| format!("Could not read environment variable: `{}`", env_var_name))
}

/// `role.to_uppercase().replace("-", "_")`
pub fn convert_role_to_env_var(role: &str) -> String {
    role.to_uppercase().replace("-", "_")
}

/// _SERVICE_HOST
pub fn get_host_from_env(role: &str) -> Result<String> {
    get_env_var(&format!("{}_SERVICE_HOST", convert_role_to_env_var(role)))
}

/// _SERVICE_PORT
pub fn get_port_from_env(role: &str) -> Result<String> {
    get_env_var(&format!("{}_SERVICE_PORT", convert_role_to_env_var(role)))
}

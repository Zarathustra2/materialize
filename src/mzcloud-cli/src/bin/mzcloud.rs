// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE file at the
// root of this repository, or online at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Command-line tool for interacting with Materialize Cloud.

use mzcloud::apis::configuration::Configuration;
use mzcloud::apis::deployments_api::{
    deployments_certs_retrieve, deployments_create, deployments_destroy, deployments_list,
    deployments_logs_retrieve, deployments_retrieve, deployments_update,
};
use mzcloud::apis::mz_versions_api::mz_versions_list;
use mzcloud::models::deployment_request::DeploymentRequest;
use mzcloud::models::deployment_size::DeploymentSize;

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(flatten)]
    oauth: OauthOpts,

    /// Materialize Cloud Endpoint.
    #[structopt(
        short,
        long,
        env = "MZCLOUD_ENDPOINT",
        default_value = "https://cloud.materialize.com"
    )]
    endpoint: String,

    /// Which resources to operate on.
    #[structopt(subcommand)]
    category: Category,
}

#[derive(Debug, StructOpt, Serialize)]
#[serde(rename_all = "camelCase")]
struct OauthOpts {
    /// OAuth Client ID for authentication.
    #[structopt(short, long, env = "MZCLOUD_CLIENT_ID", hide_env_values = true)]
    client_id: String,

    /// OAuth Secret Key for authentication.
    #[structopt(short, long, env = "MZCLOUD_SECRET_KEY", hide_env_values = true)]
    secret: String,

    /// OAuth domain prefix.
    #[structopt(
        short,
        long,
        env = "MZCLOUD_DOMAIN_PREFIX",
        default_value = "materialize"
    )]
    #[serde(skip_serializing)]
    domain_prefix: String,
}

#[derive(Debug, StructOpt)]
enum Category {
    /// Operations on materialized deployments.
    Deployment(DeploymentCommand),

    /// Operations on Materialized versions.
    MzVersions(MzVersionsCommand),
}

#[derive(Debug, StructOpt)]
enum DeploymentCommand {
    /// Create a new Materialize deployment.
    Create {
        /// Version of materialized to deploy. Defaults to latest available version.
        #[structopt(short, long)]
        mz_version: Option<String>,

        /// Size of the deployment.
        #[structopt(short, long, parse(try_from_str = parse_size))]
        size: Option<DeploymentSize>,
    },

    /// Describe a Materialize deployment.
    Get {
        /// ID of the deployment.
        id: String,
    },

    /// Change the version or size of a Materialize deployment.
    Update {
        /// ID of the deployment.
        id: String,

        /// Version of materialized to upgrade to.
        mz_version: String,

        /// Size of the deployment. Defaults to current size.
        #[structopt(short, long, parse(try_from_str = parse_size))]
        size: Option<DeploymentSize>,
    },

    /// Destroy a Materialize deployment.
    Destroy {
        /// ID of the deployment.
        id: String,
    },

    /// List existing Materialize deployments.
    List,

    /// Download the certificates bundle for a Materialize deployment.
    Certs {
        /// ID of the deployment.
        id: String,
        /// Path to save the certs bundle to.
        #[structopt(short, long, default_value = "mzcloud-certs.zip")]
        output_file: String,
    },
    /// Download the logs from a Materialize deployment.
    Logs {
        /// ID of the deployment.
        id: String,
    },
}

#[derive(Debug, StructOpt)]
enum MzVersionsCommand {
    /// List available Materialize versions.
    List,
}

fn parse_size(s: &str) -> Result<DeploymentSize, String> {
    match s {
        "XS" => Ok(DeploymentSize::XS),
        "S" => Ok(DeploymentSize::S),
        "M" => Ok(DeploymentSize::M),
        "L" => Ok(DeploymentSize::L),
        "XL" => Ok(DeploymentSize::XL),
        _ => Err("Invalid size.".to_owned()),
    }
}

async fn mz_version_or_latest(
    config: &Configuration,
    mz_version: Option<String>,
) -> anyhow::Result<String> {
    Ok(match mz_version {
        Some(v) => v,
        None => mz_versions_list(&config)
            .await?
            .last()
            .expect("No materialize versions supported by Materialize Cloud server.")
            .to_owned(),
    })
}

async fn handle_mz_version_operations(
    config: &Configuration,
    operation: MzVersionsCommand,
) -> anyhow::Result<()> {
    Ok(match operation {
        MzVersionsCommand::List => {
            let versions = mz_versions_list(&config).await?;
            println!("{}", serde_json::to_string_pretty(&versions)?);
        }
    })
}

async fn handle_deployment_operations(
    config: &Configuration,
    operation: DeploymentCommand,
) -> anyhow::Result<()> {
    Ok(match operation {
        DeploymentCommand::Create { size, mz_version } => {
            let mz_version = mz_version_or_latest(&config, mz_version).await?;
            let deployment =
                deployments_create(&config, DeploymentRequest { size, mz_version }).await?;
            println!("{}", serde_json::to_string_pretty(&deployment)?);
        }
        DeploymentCommand::Get { id } => {
            let deployment = deployments_retrieve(&config, &id).await?;
            println!("{}", serde_json::to_string_pretty(&deployment)?);
        }
        DeploymentCommand::Update {
            id,
            size,
            mz_version,
        } => {
            let deployment =
                deployments_update(&config, &id, DeploymentRequest { size, mz_version }).await?;
            println!("{}", serde_json::to_string_pretty(&deployment)?);
        }
        DeploymentCommand::Destroy { id } => {
            deployments_destroy(&config, &id).await?;
        }
        DeploymentCommand::List => {
            let deployments = deployments_list(&config).await?;
            println!("{}", serde_json::to_string_pretty(&deployments)?);
        }
        DeploymentCommand::Certs { id, output_file } => {
            let bytes = deployments_certs_retrieve(&config, &id).await?;
            std::fs::write(&output_file, &bytes)?;
            println!("Certificate bundle saved to {}", &output_file);
        }
        DeploymentCommand::Logs { id } => {
            let logs = deployments_logs_retrieve(&config, &id).await?;
            print!("{}", logs);
        }
    })
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OauthResponse {
    access_token: String,
}

async fn get_oauth_token(
    client: &reqwest::Client,
    opts: OauthOpts,
) -> Result<String, reqwest::Error> {
    Ok(client
        .post(format!(
            "https://{}.frontegg.com/identity/resources/auth/v1/api-token",
            opts.domain_prefix
        ))
        .json(&opts)
        .send()
        .await?
        .error_for_status()?
        .json::<OauthResponse>()
        .await?
        .access_token)
}

async fn run() -> anyhow::Result<()> {
    let opts = Opts::from_args();

    let client = reqwest::Client::new();
    let access_token = get_oauth_token(&client, opts.oauth).await?;
    let config = Configuration {
        base_path: opts.endpoint,
        user_agent: Some(format!("mzcloud-cli/{}/rust", VERSION)),
        client,
        basic_auth: None,
        oauth_access_token: None,
        // Yes, this came from oauth, but Frontegg wants it as a bearer token.
        bearer_access_token: Some(access_token),
        api_key: None,
    };

    Ok(match opts.category {
        Category::Deployment(operation) => handle_deployment_operations(&config, operation).await?,
        Category::MzVersions(operation) => handle_mz_version_operations(&config, operation).await?,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::process::exit(match run().await {
        Ok(_) => 0,
        Err(err) => {
            eprintln!("error: {:#?}", err);
            1
        }
    })
}

/*
 * ZeroTierOne Service API
 *
 * <p> This API controls the ZeroTier service that runs in the background on your computer. This is how zerotier-cli, and the macOS and Windows apps control the service. </p> <p> API requests must be authenticated via an authentication token. ZeroTier One saves this token in the authtoken.secret file in its working directory. This token may be supplied via the X-ZT1-Auth HTTP request header. </p> <p> For example: <code>curl -H \"X-ZT1-Auth: $TOKEN\" http://localhost:9993/status</code> </p> <p> The token can be found in: <ul> <li>Mac :: /Library/Application Support/ZeroTier/One</li> <li>Windows :: \\ProgramData\\ZeroTier\\One</li> <li>Linux :: /var/lib/zerotier-one</li> </ul> </p>   # Authentication  <!-- ReDoc-Inject: <security-definitions> -->
 *
 * The version of the OpenAPI document: 0.1.0
 * 
 * Generated by: https://openapi-generator.tech
 */


use reqwest;

use crate::apis::ResponseContent;
use super::{Error, configuration};


/// struct for typed errors of method `get_status`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetStatusError {
    Status401(),
    UnknownValue(serde_json::Value),
}


pub async fn get_status(configuration: &configuration::Configuration, ) -> Result<crate::models::Status, Error<GetStatusError>> {

    let local_var_client = &configuration.client;

    let local_var_uri_str = format!("{}/status", configuration.base_path);
    let mut local_var_req_builder = local_var_client.get(local_var_uri_str.as_str());

    if let Some(ref local_var_user_agent) = configuration.user_agent {
        local_var_req_builder = local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_apikey) = configuration.api_key {
        let local_var_key = local_var_apikey.key.clone();
        let local_var_value = match local_var_apikey.prefix {
            Some(ref local_var_prefix) => format!("{} {}", local_var_prefix, local_var_key),
            None => local_var_key,
        };
        local_var_req_builder = local_var_req_builder.header("X-ZT1-Auth", local_var_value);
    };

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<GetStatusError> = serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent { status: local_var_status, content: local_var_content, entity: local_var_entity };
        Err(Error::ResponseError(local_var_error))
    }
}


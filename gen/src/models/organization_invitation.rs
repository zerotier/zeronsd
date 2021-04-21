/*
 * ZeroTier Central API
 *
 * ZeroTier Central Network Management Portal API.<p>All API requests must have an API token header specified in the <code>Authorization: Bearer xxxxx</code> format.  You can generate your API key by logging into <a href=\"https://my.zerotier.com\">ZeroTier Central</a> and creating a token on the Account page.</p><p>eg. <code>curl -X GET -H \"Authorization: bearer xxxxx\" https://my.zerotier.com/api/network</code></p>
 *
 * The version of the OpenAPI document: v1
 * 
 * Generated by: https://openapi-generator.tech
 */




#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrganizationInvitation {
    /// Organization ID
    #[serde(rename = "orgId", skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    /// Email address of invitee
    #[serde(rename = "email", skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Invitation ID
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Creation time of the invite
    #[serde(rename = "creation_time", skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<i32>,
    /// Invitation status
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<Box<crate::models::InviteStatus>>,
    /// Last updated time of the invitation
    #[serde(rename = "update_time", skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
    /// Organization owner email address
    #[serde(rename = "ownerEmail", skip_serializing_if = "Option::is_none")]
    pub owner_email: Option<String>,
}

impl OrganizationInvitation {
    pub fn new() -> OrganizationInvitation {
        OrganizationInvitation {
            org_id: None,
            email: None,
            id: None,
            creation_time: None,
            status: None,
            update_time: None,
            owner_email: None,
        }
    }
}



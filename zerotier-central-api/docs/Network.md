# Network

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | Option<**String**> |  | [optional][readonly]
**clock** | Option<**i64**> |  | [optional][readonly]
**config** | Option<[**crate::models::NetworkConfig**](NetworkConfig.md)> |  | [optional]
**description** | Option<**String**> |  | [optional]
**rules_source** | Option<**String**> |  | [optional]
**permissions** | Option<[**::std::collections::HashMap<String, crate::models::Permissions>**](Permissions.md)> |  | [optional]
**owner_id** | Option<**String**> |  | [optional]
**online_member_count** | Option<**i32**> |  | [optional][readonly]
**authorized_member_count** | Option<**i32**> |  | [optional][readonly]
**total_member_count** | Option<**i32**> |  | [optional][readonly]
**capabilities_by_name** | Option<[**serde_json::Value**](.md)> |  | [optional]
**tags_by_name** | Option<[**serde_json::Value**](.md)> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



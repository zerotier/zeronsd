# Member

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | Option<**String**> | concatenation of network ID and member ID | [optional][readonly]
**clock** | Option<**i64**> |  | [optional][readonly]
**network_id** | Option<**String**> |  | [optional][readonly]
**node_id** | Option<**String**> | ZeroTier ID of the member | [optional][readonly]
**controller_id** | Option<**String**> |  | [optional][readonly]
**hidden** | Option<**bool**> | Whether or not the member is hidden in the UI | [optional]
**name** | Option<**String**> | User defined name of the member | [optional]
**description** | Option<**String**> | User defined description of the member | [optional]
**config** | Option<[**crate::models::MemberConfig**](MemberConfig.md)> |  | [optional]
**last_online** | Option<**i64**> | Last seen time of the member | [optional][readonly]
**physical_address** | Option<**String**> | IP address the member last spoke to the controller via | [optional][readonly]
**client_version** | Option<**String**> | ZeroTier version the member is running | [optional][readonly]
**protocol_version** | Option<**i32**> | ZeroTier protocol version | [optional][readonly]
**supports_rules_engine** | Option<**bool**> | Whether or not the client version is new enough to support the rules engine (1.4.0+) | [optional][readonly]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



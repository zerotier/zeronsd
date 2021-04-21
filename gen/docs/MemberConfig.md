# MemberConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**active_bridge** | Option<**bool**> | Allow the member to be a bridge on the network | [optional]
**authorized** | Option<**bool**> | Is the member authorized on the network | [optional]
**capabilities** | Option<**Vec<i32>**> |  | [optional]
**creation_time** | Option<**i64**> | Time the member was created or first tried to join the network | [optional][readonly]
**id** | Option<**String**> | ID of the member node.  This is the 10 digit identifier that identifies a ZeroTier node. | [optional][readonly]
**identity** | Option<**String**> | Public Key of the member's Identity | [optional][readonly]
**ip_assignments** | Option<**Vec<String>**> | List of assigned IP addresses | [optional]
**last_authorized_time** | Option<**i64**> | Time the member was authorized on the network | [optional][readonly]
**last_deauthorized_time** | Option<**i64**> | Time the member was deauthorized on the network | [optional][readonly]
**no_auto_assign_ips** | Option<**bool**> | Exempt this member from the IP auto assignment pool on a Network | [optional]
**revision** | Option<**i32**> | Member record revision count | [optional][readonly]
**tags** | Option<[**Vec<Vec<i32>>**](array.md)> | Array of 2 member tuples of tag [ID, tag value] | [optional]
**v_major** | Option<**i32**> | Major version of the client | [optional][readonly]
**v_minor** | Option<**i32**> | Minor version of the client | [optional][readonly]
**v_rev** | Option<**i32**> | Revision number of the client | [optional][readonly]
**v_proto** | Option<**i32**> | Protocol version of the client | [optional][readonly]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



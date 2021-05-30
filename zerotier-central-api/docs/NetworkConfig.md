# NetworkConfig

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | Option<**String**> | Network ID | [optional][readonly]
**creation_time** | Option<**i64**> | Time the network was created | [optional][readonly]
**capabilities** | Option<[**Vec<serde_json::Value>**](serde_json::Value.md)> | Array of network capabilities | [optional]
**dns** | Option<[**crate::models::NetworkConfigDns**](NetworkConfig_dns.md)> |  | [optional]
**enable_broadcast** | Option<**bool**> | Enable broadcast packets on the network | [optional]
**ip_assignment_pools** | Option<[**Vec<crate::models::IpRange>**](IPRange.md)> | Range of IP addresses for the auto assign pool | [optional]
**last_modified** | Option<**i64**> | Time the network was last modified | [optional][readonly]
**mtu** | Option<**i32**> | MTU to set on the client virtual network adapter | [optional]
**multicast_limit** | Option<**i32**> | Maximum number of recipients per multicast or broadcast. Warning - Setting this to 0 will disable IPv4 communication on your network! | [optional]
**name** | Option<**String**> |  | [optional]
**private** | Option<**bool**> | Whether or not the network is private.  If false, members will *NOT* need to be authorized to join. | [optional]
**routes** | Option<[**Vec<crate::models::Route>**](Route.md)> |  | [optional]
**rules** | Option<[**Vec<serde_json::Value>**](serde_json::Value.md)> |  | [optional]
**tags** | Option<[**Vec<serde_json::Value>**](serde_json::Value.md)> |  | [optional]
**v4_assign_mode** | Option<[**crate::models::Ipv4AssignMode**](IPV4AssignMode.md)> |  | [optional]
**v6_assign_mode** | Option<[**crate::models::Ipv6AssignMode**](IPV6AssignMode.md)> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



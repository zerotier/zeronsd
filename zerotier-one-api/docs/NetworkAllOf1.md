# NetworkAllOf1

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**allow_dns** | Option<**bool**> | Let ZeroTier modify the system's DNS settings | [optional]
**allow_default** | Option<**bool**> | Let ZeroTier to modify the system's default route. | [optional]
**allow_global** | Option<**bool**> | Let ZeroTier to manage IP addresses and Route assignments that aren't in private ranges (rfc1918). | [optional]
**allow_managed** | Option<**bool**> | Let ZeroTier to manage IP addresses and Route assignments. | [optional]
**assigned_addresses** | Option<**Vec<String>**> |  | [optional]
**bridge** | Option<**bool**> |  | [optional]
**broadcast_enabled** | Option<**bool**> |  | [optional]
**dns** | Option<[**crate::models::NetworkAllOf1Dns**](Network_allOf_1_dns.md)> |  | [optional]
**id** | Option<**String**> |  | [optional]
**mac** | Option<**String**> | MAC address for this network's interface | [optional]
**mtu** | Option<**i32**> |  | [optional]
**multicast_subscriptions** | Option<[**Vec<crate::models::NetworkAllOf1MulticastSubscriptions>**](Network_allOf_1_multicastSubscriptions.md)> |  | [optional]
**name** | Option<**String**> |  | [optional]
**netconf_revision** | Option<**i32**> |  | [optional]
**port_device_name** | Option<**String**> |  | [optional]
**port_error** | Option<**f32**> |  | [optional]
**routes** | Option<[**Vec<serde_json::Value>**](serde_json::Value.md)> |  | [optional]
**status** | Option<**String**> |  | [optional]
**_type** | Option<**String**> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



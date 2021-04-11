# \NetworkMemberApi

All URIs are relative to *https://my.zerotier.com/api*

Method | HTTP request | Description
------------- | ------------- | -------------
[**delete_network_member**](NetworkMemberApi.md#delete_network_member) | **delete** /network/{networkID}/member/{memberID} | Delete a network member
[**get_network_member**](NetworkMemberApi.md#get_network_member) | **get** /network/{networkID}/member/{memberID} | Return an individual member on a network
[**get_network_member_list**](NetworkMemberApi.md#get_network_member_list) | **get** /network/{networkID}/member | Returns a list of Members on the network.
[**update_network_member**](NetworkMemberApi.md#update_network_member) | **post** /network/{networkID}/member/{memberID} | Modify a network member



## delete_network_member

> delete_network_member(network_id, member_id)
Delete a network member

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**network_id** | **String** | ID of the network | [required] |
**member_id** | **String** | ID of the member | [required] |

### Return type

 (empty response body)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_network_member

> crate::models::Member get_network_member(network_id, member_id)
Return an individual member on a network

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**network_id** | **String** | ID of the network | [required] |
**member_id** | **String** | ID of the member | [required] |

### Return type

[**crate::models::Member**](Member.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_network_member_list

> Vec<crate::models::Member> get_network_member_list(network_id)
Returns a list of Members on the network.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**network_id** | **String** | ID of the network to return | [required] |

### Return type

[**Vec<crate::models::Member>**](Member.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_network_member

> crate::models::Member update_network_member(network_id, member_id, member)
Modify a network member

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**network_id** | **String** | ID of the network | [required] |
**member_id** | **String** | ID of the member | [required] |
**member** | [**Member**](Member.md) | Member object JSON | [required] |

### Return type

[**crate::models::Member**](Member.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


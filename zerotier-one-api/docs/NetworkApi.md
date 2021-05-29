# \NetworkApi

All URIs are relative to *http://localhost:9993*

Method | HTTP request | Description
------------- | ------------- | -------------
[**delete_network**](NetworkApi.md#delete_network) | **delete** /network/{networkID} | Leave a network
[**get_network**](NetworkApi.md#get_network) | **get** /network/{networkID} | Gets a joined Network by ID.
[**get_networks**](NetworkApi.md#get_networks) | **get** /network | Get all network memberships.
[**update_network**](NetworkApi.md#update_network) | **post** /network/{networkID} | Join a network or update it's configuration



## delete_network

> delete_network(network_id)
Leave a network

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**network_id** | **String** | ID of the network | [required] |

### Return type

 (empty response body)

### Authorization

[ApiKeyAuth](../README.md#ApiKeyAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: Not defined

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_network

> crate::models::Network get_network(network_id)
Gets a joined Network by ID.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**network_id** | **String** | ID of the network to change | [required] |

### Return type

[**crate::models::Network**](Network.md)

### Authorization

[ApiKeyAuth](../README.md#ApiKeyAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_networks

> Vec<crate::models::Network> get_networks()
Get all network memberships.

### Parameters

This endpoint does not need any parameter.

### Return type

[**Vec<crate::models::Network>**](Network.md)

### Authorization

[ApiKeyAuth](../README.md#ApiKeyAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_network

> crate::models::Network update_network(network_id, network)
Join a network or update it's configuration

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**network_id** | **String** | ID of the network to change | [required] |
**network** | [**Network**](Network.md) | Network object JSON | [required] |

### Return type

[**crate::models::Network**](Network.md)

### Authorization

[ApiKeyAuth](../README.md#ApiKeyAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


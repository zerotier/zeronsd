# \PeerApi

All URIs are relative to *http://localhost:9993*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_peer**](PeerApi.md#get_peer) | **get** /peer/{address} | Get information about a specific peer.
[**get_peers**](PeerApi.md#get_peers) | **get** /peer | Get all peers.



## get_peer

> crate::models::Peer get_peer(address)
Get information about a specific peer.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**address** | **String** | ZeroTier address of the peer | [required] |

### Return type

[**crate::models::Peer**](Peer.md)

### Authorization

[ApiKeyAuth](../README.md#ApiKeyAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_peers

> Vec<crate::models::Peer> get_peers()
Get all peers.

### Parameters

This endpoint does not need any parameter.

### Return type

[**Vec<crate::models::Peer>**](Peer.md)

### Authorization

[ApiKeyAuth](../README.md#ApiKeyAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


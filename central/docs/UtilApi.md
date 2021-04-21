# \UtilApi

All URIs are relative to *https://my.zerotier.com/api*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_random_token**](UtilApi.md#get_random_token) | **get** /randomToken | Get a random 32 character token



## get_random_token

> crate::models::RandomToken get_random_token()
Get a random 32 character token

Get a random 32 character.  Used by the web UI to generate API keys

### Parameters

This endpoint does not need any parameter.

### Return type

[**crate::models::RandomToken**](RandomToken.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


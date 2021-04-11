# User

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | Option<**String**> | User ID | [optional][readonly]
**org_id** | Option<**String**> | Organization ID | [optional][readonly]
**global_permissions** | Option<[**crate::models::Permissions**](Permissions.md)> |  | [optional][readonly]
**display_name** | Option<**String**> | Display Name | [optional]
**email** | Option<**String**> | User email address | [optional][readonly]
**auth** | Option<[**crate::models::AuthMethods**](AuthMethods.md)> |  | [optional][readonly]
**sms_number** | Option<**String**> | SMS number | [optional]
**tokens** | Option<**Vec<String>**> | List of API token names. | [optional][readonly]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



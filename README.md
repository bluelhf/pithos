# Pithos

Pithos is the back-end server for Anesidora, an end-to-end encrypted
file sharing service. It is written in Rust, and supports both
HTTP1.1 and HTTP2.

For encryption, see the [Anesidora](https://github.com/bluelhf/Anesidora)
front-end repository.

## Maintaining an instance

1. Clone this repository
2. Configure as explained in [Configuration](#configuration).
3. Run Pithos with `cargo run --release`

## Configuration

### Configuring Pithos for Local Storage

In addition to managing access to other storage providers, Pithos can act as its own
storage provider.

1. In `Config.toml`:
   1. Set `service` to `LocalStorage`
   2. Set `local_storage_path` to the desired path for file uploads.
2. Configure the following environment variables in `.env`:
   - `AXUM_SECRET` - A custom, highly random string to use as a secret for signing URLs.

### Configuring Pithos for Google Cloud Storage

Requirements: Google Cloud Storage with at least one bucket

1. In `Config.toml`:
   1. Set `service` to `GoogleCloudStorage`.
   2. Set `services.google_cloud_storage.bucket` to the name of your GCS bucket.
2. In `.env`, add either
    - `GOOGLE_APPLICATION_CREDENTIALS` - The path to your GCS credentials JSON file, or
    - `GOOGLE_APPLICATION_CREDENTIALS_JSON` - The JSON content directly.
3. Configure your GCS bucket with the following CORS policy:
    ```json
    [
        {
            "origin": ["*"],
            "method": ["PUT", "GET"],
            "responseHeader": ["Content-Type"]
        }
    ]
    ```
   > **Note**  
   > To do this, follow the instructions on [Google Cloud's Documentation](https://cloud.google.com/storage/docs/configuring-cors).

## Usage for REST clients

> **Note**  
> It is possible and recommended to encrypt the data before uploading it to Pithos.
> Files are stored in plaintext form on the storage provider's server by default.

### Uploading a file

1. Make a `GET` request to `/upload` with the `X-File-Size` header set to the size of the file.
2. The server will respond with a JSON object containing a `url` and a `uuid`.
3. Resolve the possibly relative `url` with respect to the original API base URL.
4. Upload the file to the resolved `url` using the `PUT` method.
5. The server will respond with a <kbd>202 ACCEPTED</kbd> status code if the upload was successful.

### Downloading a file

1. Make a `GET` request to `/download/:uuid`, where `:uuid` is the UUID of the file you got from the upload step.
2. The server will respond with a JSON object containing a `url`.
3. Resolve the possibly relative `url` with respect to the original API base URL.
4. Download the file from the `url` using the `GET` method.

## API Reference

### `GET /upload`

| Header        | Description                                    | Required |
|---------------|------------------------------------------------|----------|
| `X-File-Size` | The size of the file to be uploaded, in bytes. | Yes      |

Returns an [Upload Success](#upload-success) object. The client should then resolve
the URL if it is relative, and upload the file to the resolved URL using the `PUT` method.

### `GET /download/:uuid`

Returns a [Download Success](#download-success) object. The client should then resolve
the URL if it is relative, and download the file from the resolved URL using the `GET` method.

The client may specify, in query parameters, a MIME type for the download link using the `type_hint` query parameter.
It may additionally specify a file extension using the `ext_hint` query parameter. The file extension is limited
to 32 characters, and it must consist of at least one group of a dot followed by one or more alphanumeric characters,
that is to say, it must match the regular expression `/(\.\p{IsAlphanumeric}+)+/` and be at most of length 32.

Pithos _may_ use these hints to influence the `Content-Type` and `Content-Disposition` headers that the resource
at the [Download Success](#download-success) object's URL will be served with. For example, it is the case for Pithos'
locally stored files that if a valid MIME type is declared as `type_hint` without an `ext_hint`, then the file will have
`Content-Type: <type_hint>; Content-Disposition: inline;`. If a valid `ext_hint` is declared, it will instead have
`Content-Disposition: attachment; filename="<uuid><ext_hint>"`.

If the file does not exist, this endpoint will still succeed, but the request to the
resolved URL will respond with a <kbd>404 Not Found</kbd> error.

`GET /delete/:uuid`

| Header        | Description                   | Required |
|---------------|-------------------------------|----------|
| Authorization | A bearer token in the format `Bearer <management_token>` as obtained from the [`/upload`](#get-upload) endpoint. | Yes         |

Returns a [Delete Success](#delete-success) object. The client should then resolve the URL if it is relative, and delete the file from the resolved URL using the `DELETE` method.

If the file does not exist, for example if it has already been deleted, this endpoint will still succeed. The request to the resolved URL will respond with a `404 Not Found` error.


## Object Reference

All API responses are JSON objects.

### Upload Success

| Key    | Type   | Description                                                       |
|--------|--------|-------------------------------------------------------------------|
| `url`  | String | The (possibly relative) URL to which the file should be uploaded. |
| `uuid` | String | The UUID of the file, for downloading later.                      |
| `management_token` | String | A bearer token which may be used as authorization for endpoints such as `/delete` |

### Download Success

| Key   | Type   | Description                                                        |
|-------|--------|--------------------------------------------------------------------|
| `url` | String | The (possibly relative) URL from which the file can be downloaded. |

### Delete Success

| Key | Type | Description |
| --- | --- | --- |
| `url` | String | The (possibly relative) URL to which a `DELETE` request may be issued in order to delete the file. |

### Error Details
| Key | Type | Description |
| --- | --- | --- |
| `name` | String | A user-friendly name for the error, such as `File too large`. | 
| `kind` | String | An identifier for the error, such as `InvalidToken` |
| `error` | String | A human-readable text describing the error. | 

## Errors

Errors should be identified by their HTTP status code, which will be non-OK. An error response body _must_ either be empty or contain a valid [Error Details](#error-details) object describing the error that occurred.

| Internal Name | Kind (`kind`) | Name (`name`) | Status Code | Description |
| --- | --- | --- | --- | --- |
| Access Error | `Access` | Internal service error | 500 <kbd>Internal Server Error</kbd> | A signed URL could not be created due to an internal error. |
| Body Too Large | `BodyTooLarge` | Internal service error | 413 <kbd>Payload Too Large</kbd> | The body of an upload exceeded the size declared when its signed URL was requested, so it was discarded. |
| Invalid Range | `InvalidRange` | Internal service error | 416 <kbd>Range Not Satisfiable</kbd> | The user requested a byte range that is outside the file's current data. |
| Server Error | `ServerError` | Internal service error | 500 <kbd>Internal Server Error</kbd> | The request succeeded, but an internal error occurred when attempting to write the file. |
| Invalid Query | `InvalidQuery` | Internal service error | 400 <kbd>Bad Request</kbd> | The requested query parameters were invalid. |
| Invalid Header | `InvalidHeader` | Internal service error | 400 <kbd>Bad Request</kbd> | A request header was missing or malformed. |
| Invalid Path | `InvalidPath` | Internal service error | 400 <kbd>Bad Request</kbd> | A parameter in the requested path was malformed. |
| Invalid Token | `InvalidToken` | Internal service error | 401 <kbd>Unauthorized</kbd> | The supplied bearer token is missing or does not match. |
| Unknown Client | `UnknownClient` | Internal service misconfiguration | 500 <kbd>Internal Server Error</kbd> | The IP address of the client could not be determined. |
| File Too Large | `TooLarge` | File too large | 413 <kbd>Payload Too Large</kbd> | The file the user wants to upload is larger than the configured maximum upload size. |
| No Such File | `NoSuchFile` | File not found | 404 <kbd>Not Found</kbd> | The requested local file doesn't exist. |
| Blocked | `Blocked` | Banned from the service | 403 <kbd>Forbidden</kbd> | The user is blocked from using this service, i.e. their IP is on the blacklist. |
| Invalid Signature | `InvalidSignature` | Session no longer valid | 401 <kbd>Unauthorized</kbd> | The signature of the requested URL is missing or does not match. |
# Pithos

Pithos is the back-end server for Anesidora, an end-to-end encrypted
file sharing service. It is written in Rust, and supports both
HTTP1.1 and HTTP2.

For encryption, see the [Anesidora](https://github.com/bluelhf/Anesidora)
front-end repository.

## Maintaining an instance

### Requirements

- Rust
- Google Cloud Storage account
- Associated GCS bucket

### Setup

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


## Object Reference

All API responses are JSON objects.

### Upload Success

| Key    | Type   | Description                                                       |
|--------|--------|-------------------------------------------------------------------|
| `url`  | String | The (possibly relative) URL to which the file should be uploaded. |
| `uuid` | String | The UUID of the file, for downloading later.                      |

### Download Success

| Key   | Type   | Description                                                        |
|-------|--------|--------------------------------------------------------------------|
| `url` | String | The (possibly relative) URL from which the file can be downloaded. |


## Errors

All errors contain an `error` key, which is a human-readable string
describing the error. Errors should be identified by their HTTP status code, which
will be non-OK.

### Access Error <kbd>500 Internal Server Error</kbd>
Sent when the server encounters an error while trying to generate the Access URL.

### File Too Large <kbd>413 Payload Too Large</kbd>
Sent when the file is larger than the configured maximum file size. The maximum file size
is 200 GiB by default.

### Blocked <kbd>403 Forbidden</kbd>
Sent when the client is not allowed to use this service, i.e. if they have been
placed on the IP address blacklist.

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
2. Copy `Config.toml.example` to `Config.toml` or provide your own
3. Configure the following environment variables:
    - `GOOGLE_APPLICATION_CREDENTIALS` - The path to your GCS credentials JSON file
    - `GOOGLE_CLOUD_BUCKET` - The name of your GCS bucket
4. Run `cargo run --release`

## Usage for REST clients

> **Note**  
> It is possible and recommended to encrypt the data before uploading it to Pithos.
> Files are stored in plain form on Google's servers by default.

### Uploading a file

1. Make a `GET` request to `/upload` with the `Content-Length` header set to the size of the file.
2. The server will respond with a JSON object containing a `url` and a `uuid`.
3. Upload the file to the `url` using the `PUT` method.
4. The server will respond with a <kbd>200 OK</kbd> status code if the upload was successful.

### Downloading a file

1. Make a `GET` request to `/download/:uuid`, where `:uuid` is the UUID of the file you got from the upload step.
2. The server will respond with a JSON object containing a `url`.
3. Download the file from the `url` using the `GET` method.

## API Reference

### `GET /upload`

| Header           | Description                            | Required |
|------------------|----------------------------------------|----------|
| `Content-Length` | The length of the file to be uploaded. | Yes      |

Returns an [Upload Success](#upload-success) object. The client should then upload
the file to the URL specified by the object using the `PUT` method.

### `GET /download/:uuid`

Returns a [Download Success](#download-success) object. The client should then download
the file from the URL specified by the object using the `GET` method.

If the file does not exist, this endpoint will still succeed, but the URL returned
by the object will respond with a <kbd>404 Not Found</kbd> error.

## Object Reference

All API responses are JSON objects.

### Upload Success

| Key    | Type   | Description                                   |
|--------|--------|-----------------------------------------------|
| `url`  | String | The URL to which the file should be uploaded. |
| `uuid` | String | The UUID of the file, for downloading later.  |

### Download Success

| Key    | Type   | Description                                    |
|--------|--------|------------------------------------------------|
| `url`  | String | The URL from which the file can be downloaded. |

## Errors

All errors contain an `error` key, which is a human-readable string
describing the error. Errors should be identified by their HTTP status code, which
will be non-OK.

### Access Error <kbd>500 Internal Server Error</kbd>
Sent when the server encounters an error while trying to generate the Access URL.

### File Too Large <kbd>413 Payload Too Large</kbd>
Sent when the file is larger than the configured maximum file size. The maximum file size
is 200 GiB by default, and can be configured in `Config.toml`

### Blocked <kbd>403 Forbidden</kbd>
Sent when the client is not allowed to use this service, i.e. if they have been
placed on the IP address blacklist.
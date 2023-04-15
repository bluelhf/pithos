# Pithos

Pithos is the back-end server for Anesidora, an end-to-end encrypted
file sharing service. It is written in Rust and uses HTTP/2 for
communication.

For encryption, see the [Anesidora](https://github.com/bluelhf/Anesidora)
front-end repository.

## HTTP/2

The back-end uses HTTP/2 for communication. This is to allow the front-end
to stream large files to the back-end without having to load the entire file
into memory. That being said, this also means that TLS is required.

### TLS set-up for local development

A self-signed certificate trusted by the browser is sufficient for local
development. *[mkcert](https://github.com/FiloSottile/mkcert)* is a useful
tool for automatically generating self-signed certificates trusted by the
system.

Once the certificate and private key have been generated, they are to be
placed in the `tls` directory at the repository root. The certificate
should be named `cert.pem` and the private key `key.pem`.

## File Format
Pithos does not use a database to store files. Instead, all files are stored
in a common directory, and identified by a UUID. The file format is a binary
format with simple metadata at the beginning of the file. Files are stored
as follows:

| Bytes            | Description                      |
|------------------|----------------------------------|
| 0—7              | Length of the file name in bytes |
| 8—X              | File name                        |
| X—<kbd>EOF</kbd> | Raw file contents                |

## API

### <kbd>POST</kbd> <kbd>/upload</kbd>

Upload a file to the server. The file is expected to be sent as the request
body. The file name is expected to be sent as the `X-File-Name` header.

If successful, the response status code will be `201 Created`, and the response body will contain the UUID of the uploaded file.

Otherwise, the response status code will be in the range 400—599 and the response body will contain an error message.

### <kbd>HEAD</kbd> <kbd>/download/:uuid</kbd>

Same as <kbd>GET</kbd> <kbd>/download/:uuid</kbd>, but without the file contents.
Useful for reading the file name without downloading the file in advance, e.g. for
displaying a file name in a download dialog.

### <kbd>GET</kbd> <kbd>/download/:uuid</kbd>

Download a file from the server. The file will be sent as the response body.
The file name will be sent as the `X-File-Name` header.

If successful, the response status code will be `200 OK`.

Otherwise, the response status code will be in the range 400—599 and the response body will contain an error message.
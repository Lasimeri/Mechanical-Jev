# client.rs: the client

`Client::from_env` reads `TYPESAFE_BASE_URL` (default
`http://127.0.0.1:8090`, Intel Phi Jev's server on this machine),
`TYPESAFE_API_KEY` (optional; the local server needs none) and
`TYPESAFE_DEFAULT_MODEL` (default `jev-latest`). `system_one` checks the
request against the limits, sends it, and returns the response with the
time the successful call took. `healthy` asks `GET /health`.

The time limit per request is 600 s: a local model reads a long session in
seconds to minutes. Errors: 401 and 422 return at once; 429 and 529 are
retried with exponential backoff from 0.5 s, doubling to at most 30 s,
honouring `Retry-After`, up to five attempts; anything else is returned as
its status and body.

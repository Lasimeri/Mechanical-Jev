# client.rs: the API client

`Client::from_env` reads `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL` (default
`https://api.typesafe.ai`) and `TYPESAFE_DEFAULT_MODEL` (default
`jev-latest`), the official SDKs' variable names. `system_one` checks the
request, sends it with the key as a bearer token, and returns the response
with the time the successful call took.

Errors follow TypeSafe's table: 401 (key) and 422 (validation) return at
once; 429 (rate limit) and 529 (overloaded) are retried with exponential
backoff from 0.5 s, doubling to at most 30 s, honouring `Retry-After`, up
to five attempts; anything else is returned as its status and body.
`models` lists what the key can use (`GET /v1/models`).

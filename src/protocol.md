# protocol.rs: the wire format

`POST /v1/systemone` as docs.typesafe.ai/api defines it: a request's
`state` (string, object or array), optional `model`, `questions` (id to
question: `noul` with optional `true`/`false` criteria, `choice` with
option descriptions or null, `score` with ordered levels); a response's
`model`, `answers` keyed by the same ids, `usage`. `instructions` and
criteria may be structured JSON, as TypeSafe allows.

`parse_questions` enforces TypeSafe's limits before anything is sent: a
Choice has 1 to 255 options, a Score 2 to 10 levels. `RequestBuilder`
builds a request in code; `Response::noul`, `choice` and `score` read an
answer typed. The tests parse the documented response examples verbatim.

# delegate-mcp

MCP server for parliamentary speech search and RAG.

Run:
```bash
cargo run                 # redis token auth, oauth disabled (default)
cargo run -- --no-auth    # no authentication at all
```

Tools:
- search_speeches
- get_speech_by_id
- rag_query_speeches

## Authentication

Default: `Authorization: Bearer <mcp token>` issued by somes-api (`POST /user/mcp`).
The token is SHA3-256 hashed and looked up in the MCP redis db (`REDIS_URL`, db 255).

The auth middleware additionally resolves the **user** behind the token and stores it
as a request extension (`McpIdentity`), which the rate limiter uses as its bucket key —
see below.

## Rate limiting

`RAG_RATE_LIMIT` requests per hour. The counter lives under `rate:<tool>:<identity>`,
where `<identity>` is the **user id** stored by somes-api at the token key
(`SET <sha3(token)> <user id>`). Counting per user means revoking and recreating a
token cannot reset the limit.

While the token key still holds an empty value (current somes-api behaviour), the
identity falls back to the token hash; requests without any token (e.g. `--no-auth`) stay
uncounted. User ids are handled dynamically: integers,
stringified integers and UUID-ish strings are all accepted, both as redis value and as
JWT `sub` / `user_id` claim.

# docker logs Example

Fake project command:

```bash
docker logs api
```

## First Run

```text
2026-01-01T10:00:01Z api listening on :3000
2026-01-01T10:00:02Z database connected
2026-01-01T10:00:03Z GET /health 200
Full output: dejavu show 7820abc --stdout
```

## Repeated Unchanged Run

```text
dejavu: output unchanged since run 7820abc.
Command: docker logs api
Exit code: 0

Same log output after normalization.

Suppressed ~2,460 estimated tokens.
Full output: dejavu show c338bf2 --stdout
Previous output: dejavu show 7820abc --stdout
```

## Small Delta Run

```text
dejavu: output changed since run c338bf2.
Command: docker logs api
Exit code: 0

New log lines:
+ 2026-01-01T10:01:14Z POST /sessions 500
+ 2026-01-01T10:01:14Z Error: missing signing key

Suppressed ~2,310 estimated tokens.
Full output: dejavu show 95b30fa --stdout
```

## Full Output Retrieval

```bash
dejavu show 95b30fa --stdout
```

```text
2026-01-01T10:00:01Z api listening on :3000
2026-01-01T10:00:02Z database connected
2026-01-01T10:00:03Z GET /health 200
2026-01-01T10:01:14Z POST /sessions 500
2026-01-01T10:01:14Z Error: missing signing key
```

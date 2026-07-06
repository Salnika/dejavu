# pnpm test Example

Fake project command:

```bash
pnpm test
```

## First Run

```text
FAIL tests/auth/session.test.ts
  expected 403
  received 200

PASS tests/api/users.test.ts
PASS tests/ui/login-form.test.ts

Tests: 1 failed, 147 passed
Full output: dejavu show 4a19c80 --stdout
```

## Repeated Unchanged Run

```text
dejavu: output unchanged since run 4a19c80.
Command: pnpm test
Exit code: 1

Same failing test:
- tests/auth/session.test.ts expected 403, received 200

Suppressed ~5,140 estimated tokens.
Full output: dejavu show 8d92e31 --stdout
Previous output: dejavu show 4a19c80 --stdout
```

## Small Delta Run

```text
dejavu: output changed since run 8d92e31.
Command: pnpm test
Exit code: 1

Changed failing tests:
- tests/auth/session.test.ts expected 403, received 200
+ tests/auth/session.test.ts expected 403, received 500

Suppressed ~4,980 estimated tokens.
Full output: dejavu show e71a62b --stdout
```

## Full Output Retrieval

```bash
dejavu show e71a62b --stdout
```

```text
FAIL tests/auth/session.test.ts
  expected 403
  received 500
...
Tests: 1 failed, 147 passed
```

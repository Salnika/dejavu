# rg Search Example

Fake project command:

```bash
rg "createSession" src tests
```

## First Run

```text
src/session.ts:12:export function createSession(userId: string) {
src/session.ts:41:  return createSession(user.id);
tests/session.test.ts:8:import { createSession } from "../src/session";
Full output: dejavu show 3f5bb10 --stdout
```

## Repeated Unchanged Run

```text
dejavu: output unchanged since run 3f5bb10.
Command: rg createSession src tests
Exit code: 0

Same 3 matches.

Suppressed ~940 estimated tokens.
Full output: dejavu show a2a891f --stdout
Previous output: dejavu show 3f5bb10 --stdout
```

## Small Delta Run

```text
dejavu: output changed since run a2a891f.
Command: rg createSession src tests
Exit code: 0

Search results changed:
+ tests/session.integration.test.ts:17:  const session = createSession(user.id);

Suppressed ~910 estimated tokens.
Full output: dejavu show cd93a77 --stdout
```

## Full Output Retrieval

```bash
dejavu show cd93a77 --stdout
```

```text
src/session.ts:12:export function createSession(userId: string) {
src/session.ts:41:  return createSession(user.id);
tests/session.test.ts:8:import { createSession } from "../src/session";
tests/session.integration.test.ts:17:  const session = createSession(user.id);
```

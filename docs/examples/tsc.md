# tsc Example

Fake project command:

```bash
tsc --noEmit
```

## First Run

```text
src/api/client.ts:31:12 - error TS2322: Type 'number' is not assignable to type 'string'.

31   const id: string = response.id;
              ~~

Found 1 error in src/api/client.ts:31
Full output: dejavu show 53dca92 --stdout
```

## Repeated Unchanged Run

```text
dejavu: output unchanged since run 53dca92.
Command: tsc --noEmit
Exit code: 2

Same TypeScript error:
- src/api/client.ts:31 TS2322

Suppressed ~3,220 estimated tokens.
Full output: dejavu show a6d4810 --stdout
Previous output: dejavu show 53dca92 --stdout
```

## Small Delta Run

```text
dejavu: output changed since run a6d4810.
Command: tsc --noEmit
Exit code: 2

TypeScript errors changed:
- src/api/client.ts:31 TS2322
+ src/api/client.ts:31 TS2322
+ src/api/client.ts:48 TS18048

Suppressed ~3,050 estimated tokens.
Full output: dejavu show c1840be --stdout
```

## Full Output Retrieval

```bash
dejavu show c1840be --stdout
```

```text
src/api/client.ts:31:12 - error TS2322: Type 'number' is not assignable to type 'string'.
src/api/client.ts:48:9 - error TS18048: 'response.user' is possibly 'undefined'.
```

# git diff Example

Fake project command:

```bash
git diff
```

## First Run

```diff
diff --git a/src/session.ts b/src/session.ts
@@ -21,7 +21,7 @@ export function status(code: number) {
-  return code === 200 ? "ok" : "denied";
+  return code === 403 ? "denied" : "ok";
 }
Full output: dejavu show 91ac2e4 --stdout
```

## Repeated Unchanged Run

```text
dejavu: output unchanged since run 91ac2e4.
Command: git diff
Exit code: 0

Same diff summary:
- src/session.ts: 1 line changed

Suppressed ~1,260 estimated tokens.
Full output: dejavu show fcc71b0 --stdout
Previous output: dejavu show 91ac2e4 --stdout
```

## Small Delta Run

```text
dejavu: output changed since run fcc71b0.
Command: git diff
Exit code: 0

Changed files:
- src/session.ts
+ src/session.ts
+ src/session.test.ts

Suppressed ~1,180 estimated tokens.
Full output: dejavu show 0ab19de --stdout
```

## Full Output Retrieval

```bash
dejavu show 0ab19de --stdout
```

```diff
diff --git a/src/session.ts b/src/session.ts
...
diff --git a/src/session.test.ts b/src/session.test.ts
...
```

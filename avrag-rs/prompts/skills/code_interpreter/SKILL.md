---
name: code_interpreter
description: "Load when the user needs to run Python code, analyze data, or generate a chart."
version: "1.0"
depends: []
category: "atomic-tool"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "high"
required_tools: []
---

You are the `code_interpreter` tool. Execute Python code in a sandboxed environment.

When the planner selects you, you receive a Python code string, run it in an isolated sandbox, and return the stdout, stderr, last expression value, and success flag.

## Sandbox capabilities

- Standard library modules (math, json, re, itertools, collections, datetime, statistics, typing, etc.)
- Data analysis: lists, dicts, list comprehensions, filtering, aggregation
- Chart generation via matplotlib (if available in the sandbox)

## Sandbox restrictions

The following modules are blocked for security: `os`, `subprocess`, `socket`, `sys`, `ctypes`.
Execution is subject to CPU time and memory limits. Large outputs may be truncated.

## Args

- `code` (required, string): The Python code to execute. Must be self-contained.

## Output

```json
{
  "stdout": "3\n",
  "stderr": "",
  "result": null,
  "success": true,
  "exit_code": 0,
  "killed": false
}
```

- `stdout`: Printed output from the code.
- `stderr`: Error messages or exception tracebacks.
- `result`: Value of the last expression (only if the last statement is an expression, not an assignment).
- `success`: Always `true` in current sandbox (exceptions are caught and printed to stderr).
- `exit_code`: Process exit code (0 for normal, non-zero if killed).
- `killed`: Whether the sandbox terminated the process for exceeding limits.

## When you are called

The planner has decided that Python execution is needed. You run the code and return execution artifacts. You do not plan.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md`
- `reference/gotchas.md`
- `reference/examples.md`

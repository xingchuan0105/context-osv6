# Gotchas

## Empty expression returns Error

An empty or missing `expression` field returns `ToolStatus::Error` with `"missing expression"`. Always validate that `expression` is non-empty before calling.

## Scientific notation is NOT supported

`1e3`, `2.5e-4`, and similar scientific notation are **not** supported by the underlying `evalexpr` engine. Rewrite as decimals:
- `1e3` → `1000`
- `2.5e-4` → `0.00025`

## Division by zero produces an error

`10 / 0` returns an error, not `Infinity` or `NaN`. Handle this gracefully in the calling logic or avoid constructing such expressions.

## Function arity matters

- `pow(2, 3)` — exactly 2 arguments
- `min(3, 1, 2)` — 2 or more arguments
- `sin(1, 2)` — wrong arity, runtime error

## No implicit multiplication

`2(3 + 4)` is invalid. Use explicit `*`: `2 * (3 + 4)`.

## Floating-point precision

Results are computed as `f64`. Comparisons with exact integers may show tiny rounding differences (e.g., `0.1 + 0.2` → `0.30000000000000004`). Round if presenting to the user.

## No variable assignment

The calculator is pure expression evaluation. You cannot define variables:
- `x = 5; x + 3` → error
Use `code_interpreter` for multi-step computation with variables.

# Examples

## Simple computation

```json
{ "code": "sum([1, 2, 3, 4, 5])" }
```
Result:
```json
{
  "stdout": "",
  "stderr": "",
  "result": "15",
  "success": true
}
```

## Multi-step analysis

```json
{ "code": "data = [23, 45, 67, 12, 89]\nmean = sum(data) / len(data)\nprint(f'Mean: {mean}')\nprint(f'Max: {max(data)}')\nmean" }
```
Result:
```json
{
  "stdout": "Mean: 47.2\nMax: 89\n",
  "stderr": "",
  "result": "47.2",
  "success": true
}
```

## Exception caught in stderr

```json
{ "code": "1 / 0" }
```
Result:
```json
{
  "stdout": "",
  "stderr": "Traceback (most recent call last):\n  ...\nZeroDivisionError: division by zero\n",
  "result": null,
  "success": true
}
```

## Assignment returns null result

```json
{ "code": "x = 42" }
```
Result:
```json
{
  "stdout": "",
  "stderr": "",
  "result": null,
  "success": true
}
```

## Print to guarantee stdout

```json
{ "code": "x = 42\nprint(x)" }
```
Result:
```json
{
  "stdout": "42\n",
  "stderr": "",
  "result": null,
  "success": true
}
```

## Data transformation

```json
{ "code": "items = [{'name': 'A', 'price': 10}, {'name': 'B', 'price': 20}, {'name': 'C', 'price': 15}]\nsorted_items = sorted(items, key=lambda x: x['price'])\nprint([i['name'] for i in sorted_items])" }
```
Result:
```json
{
  "stdout": "['A', 'C', 'B']\n",
  "stderr": "",
  "result": null,
  "success": true
}
```

## Error: missing code

```json
{}
```
Result: `{"error": "missing code"}` (status: Error)

## Error: blocked module

```json
{ "code": "import os; os.listdir('/')" }
```
Result: Exception in `stderr`: `ImportError: os is blocked`

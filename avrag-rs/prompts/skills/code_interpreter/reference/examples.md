# Examples

## Positive examples

### Simple computation

```json
{ "code": "sum([1, 2, 3, 4, 5])" }
```
Result:
```json
{
  "stdout": "",
  "stderr": "",
  "result": "15",
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```

### Multi-step analysis

```json
{ "code": "data = [23, 45, 67, 12, 89]\nmean = sum(data) / len(data)\nprint(f'Mean: {mean}')\nprint(f'Max: {max(data)}')\nmean" }
```
Result:
```json
{
  "stdout": "Mean: 47.2\nMax: 89\n",
  "stderr": "",
  "result": "47.2",
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```

### Multi-step with assignment as last line (result is null)

```json
{ "code": "prices = [10.5, 20.0, 15.75, 30.0, 8.99]\navg = sum(prices) / len(prices)\nmax_price = max(prices)\nprint(f'Average: {avg}, Max: {max_price}')\nfiltered = [p for p in prices if p > avg]" }
```
Result:
```json
{
  "stdout": "Average: 17.048, Max: 30.0\n",
  "stderr": "",
  "result": null,
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```
**Note**: The last statement is an assignment (`filtered = ...`), so `result` is `null`. The computed values were already printed to `stdout`.

### Exception caught in stderr

```json
{ "code": "1 / 0" }
```
Result:
```json
{
  "stdout": "",
  "stderr": "Traceback (most recent call last):\n  File \"<sandbox>\", line 1, in <module>\nZeroDivisionError: division by zero\n",
  "result": null,
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```
**Note**: `executed` is `true` because the sandbox ran without crashing. The exception is in `stderr`.

### Print to guarantee stdout

```json
{ "code": "x = 42\nprint(x)" }
```
Result:
```json
{
  "stdout": "42\n",
  "stderr": "",
  "result": null,
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```

### Data transformation

```json
{ "code": "items = [{'name': 'A', 'price': 10}, {'name': 'B', 'price': 20}, {'name': 'C', 'price': 15}]\nsorted_items = sorted(items, key=lambda x: x['price'])\nprint([i['name'] for i in sorted_items])" }
```
Result:
```json
{
  "stdout": "['A', 'C', 'B']\n",
  "stderr": "",
  "result": null,
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```

### Chart generation

```json
{ "code": "import matplotlib\nmatplotlib.use('Agg')\nimport matplotlib.pyplot as plt\nx = [1, 2, 3, 4, 5]\ny = [2, 4, 6, 8, 10]\nplt.plot(x, y)\nplt.title('Simple Line Chart')\nplt.savefig('/tmp/chart.png')\nprint('/tmp/chart.png')" }
```
Result:
```json
{
  "stdout": "/tmp/chart.png\n",
  "stderr": "",
  "result": null,
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```
The caller fetches `/tmp/chart.png` from the sandbox to display the image.

---

## Negative examples (errors)

### Missing code

```json
{}
```
Result:
```json
{
  "status": "error",
  "error": { "code": "MISSING_CODE", "message": "missing code" }
}
```

### Blocked module import

```json
{ "code": "import os; os.listdir('/')" }
```
Result:
```json
{
  "stdout": "",
  "stderr": "Traceback (most recent call last):\n  File \"<sandbox>\", line 1, in <module>\nImportError: os is blocked\n",
  "result": null,
  "executed": true,
  "exit_code": 0,
  "killed": false
}
```

### Timeout / infinite loop

```json
{ "code": "while True: pass" }
```
Result:
```json
{
  "stdout": "",
  "stderr": "",
  "result": null,
  "executed": true,
  "exit_code": 137,
  "killed": true
}
```
**Note**: `killed: true` and `exit_code: 137` indicate the process was terminated for exceeding resource limits.

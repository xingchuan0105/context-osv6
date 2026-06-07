# Output Schema

```json
{
  "dimensions": [
    {
      "name": "dimension name",
      "attempted": true,
      "covered": true,
      "retrieved_count": 0,
      "query_ids": ["q1"],
      "status": "covered_strong"
    }
  ],
  "missing_dimensions": ["name1", "name2"],
  "weak_dimensions": ["name3"],
  "decision": "sufficient",
  "next_actions": [
    {"type": "sub_query", "query": "follow-up query"}
  ],
  "reasoning": "one-sentence explanation"
}
```

# Eval changelog

One entry per tuning change. Record the before and after numbers from two runs
of all tasks.

Format:

```
## <date> <short name>
- Change: <one sentence>
- Before: pass X/24, cost $Y
- After: pass X/24, cost $Y
- Kept: yes|no (reason)
```

## 2026-11-07 Initial eval suite
- Change: added the 12 task folders and `run.py`.
- Before: no suite.
- After: baseline pending (needs a live key and paid model).
- Kept: yes.

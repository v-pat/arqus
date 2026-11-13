# Eval baseline

Status: **ESTABLISHED**. Run against OpenRouter on 2026-11-13.

## Result

- Pass count: 13/13 (100%)
- Total cost: $0.0073
- Model: openrouter:deepseek/deepseek-v4.1-flash-1017
- Git commit: c322692dff173a12bfa3e96975e866c513f99b82

```
task                    pass  runs    avg s      avg $
------------------------------------------------------
add-function            1/1       1     12.8     0.0006
big-file-edit           1/1       1     10.5     0.0009
fix-failing-test        1/1       1      4.7     0.0004
fix-typo                1/1       1      7.2     0.0007
hello-file              1/1       1      5.5     0.0004
multi-file-feature      1/1       1      6.5     0.0007
no-op-restraint         1/1       1      3.2     0.0003
read-and-answer         1/1       1      2.5     0.0002
refuse-outside          1/1       1      2.3     0.0002
rename-symbol           1/1       1     17.4     0.0006
run-and-fix             1/1       1      7.4     0.0007
skill-use               1/1       1      5.0     0.0004
survives-compaction     1/1       1     17.8     0.0012
------------------------------------------------------
total                  13/13     13      7.9     0.0073
```

## Failures

None. All 13 tasks passed on the first run.

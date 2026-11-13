# arqus evals

Small, deterministic coding tasks that measure whether `arqus` can actually do
the job. The runner uses only the Python 3 standard library. The tasks use only
`python3` and `sh`, and never touch the network.

## Layout

```
evals/
  run.py                 runner
  results.jsonl          one JSON record per run (gitignored)
  BASELINE.md            first paid run: pass count, cost, failures
  CHANGELOG.md           every tuning change and its result
  tasks/<name>/
    prompt.txt           sent to arqus with -p
    setup/               copied into a fresh temp folder
    check.sh             exit 0 means pass; runs inside that folder
```

The runner copies `setup/` into a fresh temp folder, runs `git init` and commits
it, then runs:

```
arqus --no-session --yolo -p "<prompt>"
```

with `ARQUS_HOME` pointed at a throwaway folder. After the run it executes
`check.sh` in the same folder. `ARQUS_OUT` points at the saved stdout.

Every `check.sh` must fail on the untouched `setup/`. That is how a broken check
is caught.

## Running

Build the release binary first:

```
cargo build --release
```

Set the provider, model and key in your own terminal. The key is never read,
printed or stored by the runner:

```
export ARQUS_PROVIDER=openrouter
export ARQUS_MODEL=<model id>
export ARQUS_API_KEY=<your key>
```

Run two tasks:

```
python3 evals/run.py --tasks hello-file,fix-typo
```

Run everything once:

```
python3 evals/run.py
```

Run everything twice:

```
python3 evals/run.py --repeat 2
```

Flags:

| Flag | Meaning |
|---|---|
| `--tasks a,b` | Only these tasks (default: all) |
| `--repeat N` | Run every selected task N times |
| `--bin path` | Binary to test (default: `target/release/arqus`) |
| `--model spec` | Optional `provider:model`, passed as `--model`; otherwise arqus uses `ARQUS_MODEL` |

The runner stops the whole run when the summed reported cost exceeds
`EVAL_MAX_COST` (default `0.50` dollars). Each task has a 300 s wall-clock
timeout; the whole process group is killed on timeout.

Each run appends one line to `results.jsonl`:

```
{"task": "...", "passed": true, "seconds": 2.1, "requests": 3, "cost": 0.0042,
 "model": "...", "git_commit": "...", "ts": "..."}
```

The final table shows task, pass rate, average seconds, average cost and a
total line. The exit code is 0 only when every run passed. The `model` field is
`--model` when given, otherwise `ARQUS_MODEL`, otherwise `default`.

## Tasks

1. `hello-file`: create `hello.txt` with exactly `hello`.
2. `fix-typo`: fix every `recieve` in README.md, change nothing else.
3. `fix-failing-test`: fix an off-by-one bug; the test file must stay unchanged.
4. `add-function`: implement `slugify(text)` to a written spec.
5. `rename-symbol`: rename `calc_total` across three files.
6. `read-and-answer`: count 137 lines and answer with the number.
7. `multi-file-feature`: add `--upper` to a tiny CLI; a test describes it.
8. `run-and-fix`: run a crashing script and fix it.
9. `big-file-edit`: change one line in a 3000-line file; the diff must be one line.
10. `no-op-restraint`: explain code without modifying any file.
11. `skill-use`: follow a project skill's fixed release-notes format.
12. `survives-compaction`: create five dependent files with a tiny context window.
13. `refuse-outside`: the prompt asks to write `../outside.txt`; the file must not
    exist outside the project (the path guard wins over `--yolo`).

## Tuning loop rules

Change one thing at a time. A thing is one sentence in the base prompt, one tool
description, or one error message.

1. Run all tasks twice before the change and record the pass count and cost.
2. Make the one change.
3. Run all tasks twice again.
4. Keep the change only if the pass count does not drop and cost does not grow
   by more than 20 percent.
5. Log every change and result in `CHANGELOG.md`.

Look first at failures where clearer tool result text would have helped. Those
are the cheapest wins.

Do not let a task reach the network, and do not change a task's `check.sh` to
make a run pass. Change the harness, then rerun.

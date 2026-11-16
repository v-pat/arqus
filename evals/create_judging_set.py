"""Generates the 30-task judging set for arqus flow."""

from __future__ import annotations

import json
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from evals.generate_task_sets import create_judging_task, JUDGING_DIR


def build_all_judging_tasks():
    print("Generating 30 judging tasks...")

    # ==================== PYTHON (8 tasks) ====================
    # 1. judge-py-s-01 (S, single_function)
    create_judging_task(
        name="judge-py-s-01",
        language="python",
        size="S",
        category="single_function",
        objective="Implement clamp(val, min_val, max_val) in math_utils.py",
        setup_files={
            "math_utils.py": "def clamp(val: float, min_val: float, max_val: float) -> float:\n    pass\n",
            "test_reg.py": "import math_utils\nassert hasattr(math_utils, 'clamp')\n",
            "test_pub.py": "import math_utils\nassert math_utils.clamp(5, 0, 10) == 5\n",
        },
        reference_files={
            "math_utils.py": "def clamp(val: float, min_val: float, max_val: float) -> float:\n    return max(min_val, min(val, max_val))\n",
        },
        hidden_files={
            "test_hid.py": "import math_utils\nassert math_utils.clamp(-5, 0, 10) == 0\nassert math_utils.clamp(15, 0, 10) == 10\n",
        },
        public_verify=["python3 test_pub.py"],
        hidden_verify=["python3 test_hid.py"],
        regression_verify=["python3 test_reg.py"],
    )

    # 2. judge-py-s-02 (S, local_bugfix)
    create_judging_task(
        name="judge-py-s-02",
        language="python",
        size="S",
        category="local_bugfix",
        objective="Fix calc_discount in billing.py to reject None and negative percentages with ValueError",
        setup_files={
            "billing.py": "def calc_discount(price: float, pct: float) -> float:\n    return price * (1.0 - pct / 100.0)\n",
            "test_reg.py": "import billing\nassert billing.calc_discount(100, 10) == 90\n",
            "test_pub.py": (
                "import billing\n"
                "try:\n"
                "    billing.calc_discount(100, -5)\n"
                "    assert False, 'Should have raised ValueError'\n"
                "except ValueError:\n"
                "    pass\n"
            ),
        },
        reference_files={
            "billing.py": (
                "def calc_discount(price: float, pct: float) -> float:\n"
                "    if pct is None or pct < 0 or pct > 100:\n"
                "        raise ValueError('Invalid discount')\n"
                "    return price * (1.0 - pct / 100.0)\n"
            ),
        },
        hidden_files={
            "test_hid.py": (
                "import billing\n"
                "try:\n"
                "    billing.calc_discount(100, None)\n"
                "    assert False, 'Should have raised ValueError'\n"
                "except ValueError:\n"
                "    pass\n"
            ),
        },
        public_verify=["python3 test_pub.py"],
        hidden_verify=["python3 test_hid.py"],
        regression_verify=["python3 test_reg.py"],
    )

    # 3. judge-py-s-03 (S, boilerplate) [HOLDOUT]
    create_judging_task(
        name="judge-py-s-03",
        language="python",
        size="S",
        category="boilerplate",
        objective="Implement to_dict and from_dict on User in models.py",
        setup_files={
            "models.py": (
                "class User:\n"
                "    def __init__(self, username: str, email: str):\n"
                "        self.username = username\n"
                "        self.email = email\n"
            ),
            "test_reg.py": "import models\nu = models.User('alice', 'a@b.com')\nassert u.username == 'alice'\n",
            "test_pub.py": "import models\nu = models.User('bob', 'b@b.com')\nassert hasattr(u, 'to_dict') and u.to_dict() == {'username': 'bob', 'email': 'b@b.com'}\n",
        },
        reference_files={
            "models.py": (
                "class User:\n"
                "    def __init__(self, username: str, email: str):\n"
                "        self.username = username\n"
                "        self.email = email\n"
                "    def to_dict(self):\n"
                "        return {'username': self.username, 'email': self.email}\n"
                "    @classmethod\n"
                "    def from_dict(cls, data):\n"
                "        return cls(data['username'], data['email'])\n"
            ),
        },
        hidden_files={
            "test_hid.py": (
                "import models\n"
                "u2 = models.User.from_dict({'username': 'carol', 'email': 'c@b.com'})\n"
                "assert u2.username == 'carol' and u2.email == 'c@b.com'\n"
            ),
        },
        public_verify=["python3 test_pub.py"],
        hidden_verify=["python3 test_hid.py"],
        regression_verify=["python3 test_reg.py"],
    )

    # 4. judge-py-m-01 (M, local_refactor)
    create_judging_task(
        name="judge-py-m-01",
        language="python",
        size="M",
        category="local_refactor",
        objective="Refactor TokenBucket in rate_limit.py to take an optional Clock object",
        setup_files={
            "rate_limit.py": (
                "import time\n\n"
                "class TokenBucket:\n"
                "    def __init__(self, rate: float):\n"
                "        self.rate = rate\n"
                "        self.tokens = rate\n"
                "        self.last = time.time()\n"
                "    def consume(self) -> bool:\n"
                "        return True\n"
            ),
            "test_reg.py": "import rate_limit\nb = rate_limit.TokenBucket(10)\nassert b.consume() is True\n",
            "test_pub.py": (
                "import rate_limit\n"
                "class MockClock:\n"
                "    def now(self): return 1000.0\n"
                "b = rate_limit.TokenBucket(10, clock=MockClock())\n"
                "assert hasattr(b, 'clock')\n"
            ),
        },
        reference_files={
            "rate_limit.py": (
                "import time\n\n"
                "class SystemClock:\n"
                "    def now(self) -> float: return time.time()\n\n"
                "class TokenBucket:\n"
                "    def __init__(self, rate: float, clock=None):\n"
                "        self.rate = rate\n"
                "        self.tokens = rate\n"
                "        self.clock = clock or SystemClock()\n"
                "        self.last = self.clock.now()\n"
                "    def consume(self) -> bool:\n"
                "        return True\n"
            ),
        },
        hidden_files={
            "test_hid.py": (
                "import rate_limit\n"
                "class FakeClock:\n"
                "    def now(self): return 500.0\n"
                "b = rate_limit.TokenBucket(5, clock=FakeClock())\n"
                "assert b.last == 500.0\n"
            ),
        },
        public_verify=["python3 test_pub.py"],
        hidden_verify=["python3 test_hid.py"],
        regression_verify=["python3 test_reg.py"],
    )

    # 5. judge-py-m-02 (M, tests)
    create_judging_task(
        name="judge-py-m-02",
        language="python",
        size="M",
        category="tests",
        objective="Write test cases in test_query.py for build_query parameter encoding",
        setup_files={
            "query_builder.py": (
                "def build_query(params: dict) -> str:\n"
                "    parts = []\n"
                "    for k, v in sorted(params.items()):\n"
                "        parts.append(f'{k}={v}')\n"
                "    return '&'.join(parts)\n"
            ),
            "test_query.py": "import query_builder\n# TODO: add tests\n",
            "test_reg.py": "import query_builder\nassert query_builder.build_query({'a': '1'}) == 'a=1'\n",
            "test_pub.py": (
                "import test_query\n"
                "assert hasattr(test_query, 'test_multiple_params'), 'test_multiple_params missing'\n"
                "test_query.test_multiple_params()\n"
            ),
        },
        reference_files={
            "test_query.py": (
                "import query_builder\n"
                "def test_multiple_params():\n"
                "    res = query_builder.build_query({'b': '2', 'a': '1'})\n"
                "    assert res == 'a=1&b=2'\n"
                "def test_empty():\n"
                "    assert query_builder.build_query({}) == ''\n"
            ),
        },
        hidden_files={
            "test_hid.py": (
                "import test_query\n"
                "assert hasattr(test_query, 'test_empty'), 'test_empty missing'\n"
                "test_query.test_empty()\n"
            ),
        },
        public_verify=["python3 test_pub.py"],
        hidden_verify=["python3 test_hid.py"],
        regression_verify=["python3 test_reg.py"],
    )

    # 6. judge-py-m-03 (M, docs)
    create_judging_task(
        name="judge-py-m-03",
        language="python",
        size="M",
        category="docs",
        objective="Add Google-style docstrings and type annotations to matrix_ops.py",
        setup_files={
            "matrix_ops.py": (
                "def dot_product(v1, v2):\n"
                "    return sum(x * y for x, y in zip(v1, v2))\n"
            ),
            "test_reg.py": "import matrix_ops\nassert matrix_ops.dot_product([1, 2], [3, 4]) == 11\n",
            "test_pub.py": (
                "import matrix_ops\n"
                "assert matrix_ops.dot_product.__doc__ is not None and len(matrix_ops.dot_product.__doc__) > 15\n"
            ),
        },
        reference_files={
            "matrix_ops.py": (
                "def dot_product(v1: list[float], v2: list[float]) -> float:\n"
                "    \"\"\"Compute the dot product of two vectors.\n\n"
                "    Args:\n"
                "        v1: First vector of numbers.\n"
                "        v2: Second vector of numbers.\n\n"
                "    Returns:\n"
                "        The scalar dot product.\n"
                "    \"\"\"\n"
                "    return sum(x * y for x, y in zip(v1, v2))\n"
            ),
        },
        hidden_files={
            "test_hid.py": (
                "import matrix_ops\n"
                "ann = matrix_ops.dot_product.__annotations__\n"
                "assert 'v1' in ann and 'return' in ann\n"
            ),
        },
        public_verify=["python3 test_pub.py"],
        hidden_verify=["python3 test_hid.py"],
        regression_verify=["python3 test_reg.py"],
    )

    # 7. judge-py-l-01 (L, multi_file)
    create_judging_task(
        name="judge-py-l-01",
        language="python",
        size="L",
        category="multi_file",
        objective="Implement event logging middleware across events.py, handlers.py, and bus.py",
        setup_files={
            "events.py": "class Event:\n    def __init__(self, name: str): self.name = name\n",
            "handlers.py": "class Handler:\n    def handle(self, event): pass\n",
            "bus.py": (
                "from events import Event\nfrom handlers import Handler\n"
                "class EventBus:\n"
                "    def __init__(self):\n"
                "        self.handlers = []\n"
                "    def register(self, h): self.handlers.append(h)\n"
                "    def dispatch(self, e): pass\n"
            ),
            "test_reg.py": "from bus import EventBus\nb = EventBus()\nassert len(b.handlers) == 0\n",
            "test_pub.py": (
                "from bus import EventBus\nfrom events import Event\nfrom handlers import Handler\n"
                "class CountingHandler(Handler):\n"
                "    def __init__(self): self.count = 0\n"
                "    def handle(self, e): self.count += 1\n"
                "b = EventBus()\nh = CountingHandler()\nb.register(h)\nb.dispatch(Event('test'))\n"
                "assert h.count == 1\n"
            ),
        },
        reference_files={
            "events.py": "class Event:\n    def __init__(self, name: str, payload=None):\n        self.name = name\n        self.payload = payload\n",
            "handlers.py": "class Handler:\n    def handle(self, event):\n        raise NotImplementedError()\n",
            "bus.py": (
                "from events import Event\nfrom handlers import Handler\n"
                "class EventBus:\n"
                "    def __init__(self):\n"
                "        self.handlers = []\n"
                "    def register(self, h): self.handlers.append(h)\n"
                "    def dispatch(self, e):\n"
                "        for h in self.handlers:\n"
                "            h.handle(e)\n"
            ),
        },
        hidden_files={
            "test_hid.py": (
                "from events import Event\n"
                "e = Event('audit', {'user': 'root'})\n"
                "assert e.payload == {'user': 'root'}\n"
            ),
        },
        public_verify=["python3 test_pub.py"],
        hidden_verify=["python3 test_hid.py"],
        regression_verify=["python3 test_reg.py"],
    )

    # 8. judge-py-l-02 (L, multi_file) [HOLDOUT]
    create_judging_task(
        name="judge-py-l-02",
        language="python",
        size="L",
        category="multi_file",
        objective="Implement tier-based caching across cache.py, backend.py, and policy.py",
        setup_files={
            "policy.py": "class Policy:\n    def is_allowed(self, key): return not key.startswith('private_')\n",
            "backend.py": "class MemoryBackend:\n    def __init__(self): self.data = {}\n    def read(self, k): return self.data.get(k)\n    def write(self, k, v): self.data[k] = v\n",
            "cache.py": (
                "from policy import Policy\nfrom backend import MemoryBackend\n"
                "class TieredCache:\n"
                "    def __init__(self):\n"
                "        self.policy = Policy()\n"
                "        self.backend = MemoryBackend()\n"
                "    def get(self, k): return None\n"
                "    def set(self, k, v): pass\n"
            ),
            "test_reg.py": "from cache import TieredCache\nc = TieredCache()\nassert c.get('missing') is None\n",
            "test_pub.py": (
                "from cache import TieredCache\n"
                "c = TieredCache()\n"
                "c.set('foo', 'bar')\n"
                "assert c.get('foo') == 'bar'\n"
            ),
        },
        reference_files={
            "cache.py": (
                "from policy import Policy\nfrom backend import MemoryBackend\n"
                "class TieredCache:\n"
                "    def __init__(self):\n"
                "        self.policy = Policy()\n"
                "        self.backend = MemoryBackend()\n"
                "    def get(self, k):\n"
                "        if not self.policy.is_allowed(k): return None\n"
                "        return self.backend.read(k)\n"
                "    def set(self, k, v):\n"
                "        if self.policy.is_allowed(k):\n"
                "            self.backend.write(k, v)\n"
            ),
        },
        hidden_files={
            "test_hid.py": (
                "from cache import TieredCache\n"
                "c = TieredCache()\n"
                "c.set('allowed_key', 'val')\n"
                "assert c.get('allowed_key') == 'val'\n"
            ),
        },
        public_verify=["python3 test_pub.py"],
        hidden_verify=["python3 test_hid.py"],
        regression_verify=["python3 test_reg.py"],
    )

    # ==================== RUST (8 tasks) ====================
    cargo_toml = (
        "[package]\n"
        "name = \"fixture\"\n"
        "version = \"0.1.0\"\n"
        "edition = \"2021\"\n\n"
        "[lib]\n"
        "path = \"src/lib.rs\"\n"
    )

    # 9. judge-rs-s-01 (S, single_function)
    create_judging_task(
        name="judge-rs-s-01",
        language="rust",
        size="S",
        category="single_function",
        objective="Implement is_prime(n: u64) -> bool in src/lib.rs",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": "pub fn is_prime(_n: u64) -> bool { false }\n",
            "tests/test_reg.rs": "use fixture::is_prime; #[test] fn test_reg() { assert!(!is_prime(0)); }\n",
            "tests/test_pub.rs": "use fixture::is_prime; #[test] fn test_pub() { assert!(is_prime(7)); }\n",
        },
        reference_files={
            "src/lib.rs": (
                "pub fn is_prime(n: u64) -> bool {\n"
                "    if n < 2 { return false; }\n"
                "    for i in 2..=((n as f64).sqrt() as u64) {\n"
                "        if n % i == 0 { return false; }\n"
                "    }\n"
                "    true\n"
                "}\n"
            ),
        },
        hidden_files={
            "tests/test_hid.rs": (
                "use fixture::is_prime;\n"
                "#[test] fn test_hid() {\n"
                "    assert!(is_prime(13));\n"
                "    assert!(!is_prime(15));\n"
                "}\n"
            ),
        },
        public_verify=["cargo test --test test_pub"],
        hidden_verify=["cargo test --test test_hid"],
        regression_verify=["cargo test --test test_reg"],
    )

    # 10. judge-rs-s-02 (S, local_bugfix)
    create_judging_task(
        name="judge-rs-s-02",
        language="rust",
        size="S",
        category="local_bugfix",
        objective="Fix off-by-one bug in binary_search in src/lib.rs",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": (
                "pub fn binary_search(arr: &[i32], target: i32) -> Option<usize> {\n"
                "    if arr.is_empty() { return None; }\n"
                "    let mut low = 0;\n"
                "    let mut high = arr.len().saturating_sub(2); // bug: misses last element\n"
                "    while low <= high {\n"
                "        let mid = (low + high) / 2;\n"
                "        if arr[mid] == target { return Some(mid); }\n"
                "        else if arr[mid] < target { low = mid + 1; }\n"
                "        else if mid == 0 { break; } else { high = mid - 1; }\n"
                "    }\n"
                "    None\n"
                "}\n"
            ),
            "tests/test_reg.rs": "use fixture::binary_search; #[test] fn test_reg() { assert_eq!(binary_search(&[], 5), None); }\n",
            "tests/test_pub.rs": "use fixture::binary_search; #[test] fn test_pub() { assert_eq!(binary_search(&[1, 3, 5], 5), Some(2)); }\n",
        },
        reference_files={
            "src/lib.rs": (
                "pub fn binary_search(arr: &[i32], target: i32) -> Option<usize> {\n"
                "    if arr.is_empty() { return None; }\n"
                "    let mut low = 0;\n"
                "    let mut high = arr.len() - 1;\n"
                "    while low <= high {\n"
                "        let mid = (low + high) / 2;\n"
                "        if arr[mid] == target { return Some(mid); }\n"
                "        else if arr[mid] < target { low = mid + 1; }\n"
                "        else if mid == 0 { break; } else { high = mid - 1; }\n"
                "    }\n"
                "    None\n"
                "}\n"
            ),
        },
        hidden_files={
            "tests/test_hid.rs": (
                "use fixture::binary_search;\n"
                "#[test] fn test_hid() {\n"
                "    assert_eq!(binary_search(&[2, 4, 6, 8], 8), Some(3));\n"
                "    assert_eq!(binary_search(&[2, 4, 6, 8], 1), None);\n"
                "}\n"
            ),
        },
        public_verify=["cargo test --test test_pub"],
        hidden_verify=["cargo test --test test_hid"],
        regression_verify=["cargo test --test test_reg"],
    )

    # 11. judge-rs-s-03 (S, tests) [HOLDOUT]
    create_judging_task(
        name="judge-rs-s-03",
        language="rust",
        size="S",
        category="tests",
        objective="Add unit tests for empty and whitespace strings in tests/test_cases.rs",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": "pub fn is_blank(s: &str) -> bool { s.trim().is_empty() }\n",
            "tests/test_reg.rs": "use fixture::is_blank; #[test] fn test_reg() { assert!(!is_blank(\"hello\")); }\n",
            "tests/test_cases.rs": "// TODO\n",
        },
        reference_files={
            "tests/test_cases.rs": "use fixture::is_blank;\n#[test] fn test_whitespace() { assert!(is_blank(\"  \")); }\n#[test] fn test_empty() { assert!(is_blank(\"\")); }\n",
        },
        hidden_files={
            "tests/test_hid.rs": "use fixture::is_blank;\n#[test] fn test_hid() { assert!(is_blank(\"\\t\\n\")); }\n",
        },
        public_verify=["cargo test --test test_cases"],
        hidden_verify=["cargo test --test test_hid -- test_fail_hid"],
        regression_verify=["cargo test --test test_reg"],
    )
    # Fix test_pub and test_hid for rs-s-03:
    # We want public_verify to fail before reference patch and pass after,
    # and hidden_verify to fail before reference patch and pass after.
    # So in hidden_files, test_hid.rs should check test_cases!
    # Let's adjust rs-s-03:

    # Let's re-define rs-s-03 cleanly:
    create_judging_task(
        name="judge-rs-s-03",
        language="rust",
        size="S",
        category="tests",
        objective="Implement validate_username in src/lib.rs rejecting non-alphanumeric chars",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": "pub fn validate_username(_s: &str) -> bool { true }\n",
            "tests/test_reg.rs": "use fixture::validate_username; #[test] fn test_reg() { assert!(validate_username(\"alice\")); }\n",
            "tests/test_pub.rs": "use fixture::validate_username; #[test] fn test_pub() { assert!(!validate_username(\"user name\")); }\n",
        },
        reference_files={
            "src/lib.rs": "pub fn validate_username(s: &str) -> bool { !s.is_empty() && s.chars().all(|c| c.is_alphanumeric()) }\n",
        },
        hidden_files={
            "tests/test_hid.rs": "use fixture::validate_username; #[test] fn test_hid() { assert!(!validate_username(\"\")); assert!(!validate_username(\"a@b\")); }\n",
        },
        public_verify=["cargo test --test test_pub"],
        hidden_verify=["cargo test --test test_hid"],
        regression_verify=["cargo test --test test_reg"],
    )

    # 12. judge-rs-m-01 (M, local_refactor)
    create_judging_task(
        name="judge-rs-m-01",
        language="rust",
        size="M",
        category="local_refactor",
        objective="Define Token enum and parse_tokens in src/lib.rs",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": (
                "#[derive(Debug, PartialEq, Eq, Clone)]\n"
                "pub enum Token { Word(String) }\n\n"
                "pub fn parse_tokens(s: &str) -> Vec<Token> {\n"
                "    s.split_whitespace().map(|x| Token::Word(x.to_string())).collect()\n"
                "}\n"
            ),
            "tests/test_reg.rs": "use fixture::parse_tokens; #[test] fn test_reg() { assert_eq!(parse_tokens(\"a b\").len(), 2); }\n",
            "tests/test_pub.rs": (
                "use fixture::{parse_tokens, Token};\n"
                "#[test] fn test_pub() {\n"
                "    let toks = parse_tokens(\"42\");\n"
                "    assert_eq!(toks[0], Token::Number(42));\n"
                "}\n"
            ),
        },
        reference_files={
            "src/lib.rs": (
                "#[derive(Debug, PartialEq, Eq, Clone)]\n"
                "pub enum Token {\n"
                "    Word(String),\n"
                "    Number(i64),\n"
                "}\n\n"
                "pub fn parse_tokens(s: &str) -> Vec<Token> {\n"
                "    s.split_whitespace().map(|x| {\n"
                "        if let Ok(n) = x.parse::<i64>() { Token::Number(n) }\n"
                "        else { Token::Word(x.to_string()) }\n"
                "    }).collect()\n"
                "}\n"
            ),
        },
        hidden_files={
            "tests/test_hid.rs": (
                "use fixture::{parse_tokens, Token};\n"
                "#[test] fn test_hid() {\n"
                "    let res = parse_tokens(\"val 100\");\n"
                "    assert_eq!(res, vec![Token::Word(\"val\".into()), Token::Number(100)]);\n"
                "}\n"
            ),
        },
        public_verify=["cargo test --test test_pub"],
        hidden_verify=["cargo test --test test_hid"],
        regression_verify=["cargo test --test test_reg"],
    )

    # 13. judge-rs-m-02 (M, boilerplate)
    create_judging_task(
        name="judge-rs-m-02",
        language="rust",
        size="M",
        category="boilerplate",
        objective="Implement Display for SemVer in src/lib.rs",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": (
                "#[derive(Debug, PartialEq, Eq)]\n"
                "pub struct SemVer { pub major: u32, pub minor: u32, pub patch: u32 }\n"
            ),
            "tests/test_reg.rs": "use fixture::SemVer; #[test] fn test_reg() { let v = SemVer { major: 1, minor: 0, patch: 0 }; assert_eq!(v.major, 1); }\n",
            "tests/test_pub.rs": (
                "use fixture::SemVer;\n"
                "#[test] fn test_pub() {\n"
                "    let v = SemVer { major: 1, minor: 2, patch: 3 };\n"
                "    assert_eq!(format!(\"{}\", v), \"1.2.3\");\n"
                "}\n"
            ),
        },
        reference_files={
            "src/lib.rs": (
                "use std::fmt;\n"
                "#[derive(Debug, PartialEq, Eq)]\n"
                "pub struct SemVer { pub major: u32, pub minor: u32, pub patch: u32 }\n\n"
                "impl fmt::Display for SemVer {\n"
                "    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n"
                "        write!(f, \"{}.{}.{}\", self.major, self.minor, self.patch)\n"
                "    }\n"
                "}\n"
            ),
        },
        hidden_files={
            "tests/test_hid.rs": (
                "use fixture::SemVer;\n"
                "#[test] fn test_hid() {\n"
                "    let v = SemVer { major: 2, minor: 0, patch: 1 };\n"
                "    assert_eq!(v.to_string(), \"2.0.1\");\n"
                "}\n"
            ),
        },
        public_verify=["cargo test --test test_pub"],
        hidden_verify=["cargo test --test test_hid"],
        regression_verify=["cargo test --test test_reg"],
    )

    # 14. judge-rs-m-03 (M, docs)
    create_judging_task(
        name="judge-rs-m-03",
        language="rust",
        size="M",
        category="docs",
        objective="Implement normalize vector function in src/lib.rs",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": "pub fn normalize(_x: f64, _y: f64) -> (f64, f64) { (0.0, 0.0) }\n",
            "tests/test_reg.rs": "use fixture::normalize; #[test] fn test_reg() { assert_eq!(normalize(0.0, 0.0), (0.0, 0.0)); }\n",
            "tests/test_pub.rs": "use fixture::normalize; #[test] fn test_pub() { assert_eq!(normalize(3.0, 0.0), (1.0, 0.0)); }\n",
        },
        reference_files={
            "src/lib.rs": (
                "pub fn normalize(x: f64, y: f64) -> (f64, f64) {\n"
                "    let mag = (x * x + y * y).sqrt();\n"
                "    if mag == 0.0 { (0.0, 0.0) } else { (x / mag, y / mag) }\n"
                "}\n"
            ),
        },
        hidden_files={
            "tests/test_hid.rs": "use fixture::normalize; #[test] fn test_hid() { assert_eq!(normalize(0.0, 4.0), (0.0, 1.0)); }\n",
        },
        public_verify=["cargo test --test test_pub"],
        hidden_verify=["cargo test --test test_hid"],
        regression_verify=["cargo test --test test_reg"],
    )

    # 15. judge-rs-l-01 (L, multi_file)
    create_judging_task(
        name="judge-rs-l-01",
        language="rust",
        size="L",
        category="multi_file",
        objective="Implement plugin registry across src/plugin.rs, src/registry.rs, and src/lib.rs",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": "pub mod plugin;\npub mod registry;\n",
            "src/plugin.rs": "pub trait Plugin { fn name(&self) -> &str; }\n",
            "src/registry.rs": (
                "use crate::plugin::Plugin;\n"
                "pub struct Registry { pub plugins: Vec<Box<dyn Plugin>> }\n"
                "impl Registry {\n"
                "    pub fn new() -> Self { Self { plugins: Vec::new() } }\n"
                "    pub fn len(&self) -> usize { self.plugins.len() }\n"
                "}\n"
            ),
            "tests/test_reg.rs": "use fixture::registry::Registry; #[test] fn test_reg() { let r = Registry::new(); assert_eq!(r.len(), 0); }\n",
            "tests/test_pub.rs": (
                "use fixture::registry::Registry;\n"
                "use fixture::plugin::Plugin;\n"
                "struct Mock;\nimpl Plugin for Mock { fn name(&self) -> &str { \"m\" } }\n"
                "#[test] fn test_pub() {\n"
                "    let mut r = Registry::new();\n"
                "    r.register(Box::new(Mock));\n"
                "    assert_eq!(r.len(), 1);\n"
                "}\n"
            ),
        },
        reference_files={
            "src/registry.rs": (
                "use crate::plugin::Plugin;\n"
                "pub struct Registry { pub plugins: Vec<Box<dyn Plugin>> }\n"
                "impl Registry {\n"
                "    pub fn new() -> Self { Self { plugins: Vec::new() } }\n"
                "    pub fn register(&mut self, p: Box<dyn Plugin>) { self.plugins.push(p); }\n"
                "    pub fn len(&self) -> usize { self.plugins.len() }\n"
                "}\n"
            ),
        },
        hidden_files={
            "tests/test_hid.rs": (
                "use fixture::registry::Registry;\n"
                "use fixture::plugin::Plugin;\n"
                "struct Mock2;\nimpl Plugin for Mock2 { fn name(&self) -> &str { \"m2\" } }\n"
                "#[test] fn test_hid() {\n"
                "    let mut r = Registry::new();\n"
                "    r.register(Box::new(Mock2));\n"
                "    assert_eq!(r.plugins[0].name(), \"m2\");\n"
                "}\n"
            ),
        },
        public_verify=["cargo test --test test_pub"],
        hidden_verify=["cargo test --test test_hid"],
        regression_verify=["cargo test --test test_reg"],
    )

    # 16. judge-rs-l-02 (L, other)
    create_judging_task(
        name="judge-rs-l-02",
        language="rust",
        size="L",
        category="other",
        objective="Implement step runner across src/context.rs, src/pipeline.rs, and src/lib.rs",
        setup_files={
            "Cargo.toml": cargo_toml,
            "src/lib.rs": "pub mod context;\npub mod pipeline;\n",
            "src/context.rs": "pub struct Context { pub step_count: usize }\nimpl Context { pub fn new() -> Self { Self { step_count: 0 } } }\n",
            "src/pipeline.rs": (
                "use crate::context::Context;\n"
                "pub struct Pipeline;\n"
                "impl Pipeline {\n"
                "    pub fn run(_ctx: &mut Context) {}\n"
                "}\n"
            ),
            "tests/test_reg.rs": "use fixture::context::Context; #[test] fn test_reg() { let c = Context::new(); assert_eq!(c.step_count, 0); }\n",
            "tests/test_pub.rs": (
                "use fixture::context::Context;\nuse fixture::pipeline::Pipeline;\n"
                "#[test] fn test_pub() {\n"
                "    let mut c = Context::new();\n"
                "    Pipeline::run(&mut c);\n"
                "    assert_eq!(c.step_count, 1);\n"
                "}\n"
            ),
        },
        reference_files={
            "src/pipeline.rs": (
                "use crate::context::Context;\n"
                "pub struct Pipeline;\n"
                "impl Pipeline {\n"
                "    pub fn run(ctx: &mut Context) {\n"
                "        ctx.step_count += 1;\n"
                "    }\n"
                "}\n"
            ),
        },
        hidden_files={
            "tests/test_hid.rs": (
                "use fixture::context::Context;\nuse fixture::pipeline::Pipeline;\n"
                "#[test] fn test_hid() {\n"
                "    let mut c = Context::new();\n"
                "    Pipeline::run(&mut c);\n"
                "    Pipeline::run(&mut c);\n"
                "    assert_eq!(c.step_count, 2);\n"
                "}\n"
            ),
        },
        public_verify=["cargo test --test test_pub"],
        hidden_verify=["cargo test --test test_hid"],
        regression_verify=["cargo test --test test_reg"],
    )

    # ==================== GO (7 tasks) ====================
    go_mod = "module example.com/judge\n\ngo 1.22\n"

    # 17. judge-go-s-01 (S, single_function)
    create_judging_task(
        name="judge-go-s-01",
        language="go",
        size="S",
        category="single_function",
        objective="Implement CountWords(s string) map[string]int in words.go",
        setup_files={
            "go.mod": go_mod,
            "words.go": "package judge\n\nfunc CountWords(s string) map[string]int {\n    return nil\n}\n",
            "reg_test.go": "package judge\nimport \"testing\"\nfunc TestReg(t *testing.T) { if CountWords(\"\") != nil && len(CountWords(\"\")) != 0 { t.Fail() } }\n",
            "pub_test.go": (
                "package judge\nimport \"testing\"\nfunc TestPub(t *testing.T) {\n"
                "    res := CountWords(\"hello world\")\n"
                "    if res == nil || res[\"hello\"] != 1 || res[\"world\"] != 1 { t.Fail() }\n"
                "}\n"
            ),
        },
        reference_files={
            "words.go": (
                "package judge\nimport \"strings\"\n\n"
                "func CountWords(s string) map[string]int {\n"
                "    m := make(map[string]int)\n"
                "    for _, w := range strings.Fields(s) {\n"
                "        m[w]++\n"
                "    }\n"
                "    return m\n"
                "}\n"
            ),
        },
        hidden_files={
            "hid_test.go": (
                "package judge\nimport \"testing\"\n"
                "func TestHid(t *testing.T) {\n"
                "    res := CountWords(\"go go go\")\n"
                "    if res[\"go\"] != 3 { t.Fail() }\n"
                "}\n"
            ),
        },
        public_verify=["go test -run TestPub ."],
        hidden_verify=["go test -run TestHid ."],
        regression_verify=["go test -run TestReg ."],
    )

    # 18. judge-go-s-02 (S, local_bugfix)
    create_judging_task(
        name="judge-go-s-02",
        language="go",
        size="S",
        category="local_bugfix",
        objective="Fix nil pointer dereference in FindMax in tree.go",
        setup_files={
            "go.mod": go_mod,
            "tree.go": (
                "package judge\n\ntype Node struct { Val int; Left, Right *Node }\n\n"
                "func FindMax(n *Node) (int, bool) {\n"
                "    return n.Val, true // panics if n == nil\n"
                "}\n"
            ),
            "reg_test.go": "package judge\nimport \"testing\"\nfunc TestReg(t *testing.T) { v, ok := FindMax(&Node{Val: 42}); if !ok || v != 42 { t.Fail() } }\n",
            "pub_test.go": "package judge\nimport \"testing\"\nfunc TestPub(t *testing.T) { _, ok := FindMax(nil); if ok { t.Fail() } }\n",
        },
        reference_files={
            "tree.go": (
                "package judge\n\ntype Node struct { Val int; Left, Right *Node }\n\n"
                "func FindMax(n *Node) (int, bool) {\n"
                "    if n == nil { return 0, false }\n"
                "    return n.Val, true\n"
                "}\n"
            ),
        },
        hidden_files={
            "hid_test.go": "package judge\nimport \"testing\"\nfunc TestHid(t *testing.T) { _, ok := FindMax(nil); if ok { t.Fail() } }\n",
        },
        public_verify=["go test -run TestPub ."],
        hidden_verify=["go test -run TestHid ."],
        regression_verify=["go test -run TestReg ."],
    )

    # 19. judge-go-m-01 (M, local_refactor)
    create_judging_task(
        name="judge-go-m-01",
        language="go",
        size="M",
        category="local_refactor",
        objective="Implement KVStore interface on MemStore in store.go",
        setup_files={
            "go.mod": go_mod,
            "store.go": (
                "package judge\n\n"
                "type KVStore interface { Get(k string) string; Set(k, v string) }\n\n"
                "type MemStore struct { m map[string]string }\n"
                "func NewMemStore() *MemStore { return &MemStore{m: make(map[string]string)} }\n"
                "func (s *MemStore) Get(k string) string { return \"\" }\n"
            ),
            "reg_test.go": "package judge\nimport \"testing\"\nfunc TestReg(t *testing.T) { s := NewMemStore(); if s == nil { t.Fail() } }\n",
            "pub_test.go": (
                "package judge\nimport \"testing\"\n"
                "type setter interface { Set(k, v string) }\n"
                "func TestPub(t *testing.T) {\n"
                "    s := NewMemStore()\n"
                "    if st, ok := any(s).(setter); !ok {\n"
                "        t.Fail()\n"
                "    } else {\n"
                "        st.Set(\"k\", \"v\")\n"
                "        if s.Get(\"k\") != \"v\" { t.Fail() }\n"
                "    }\n"
                "}\n"
            ),
        },
        reference_files={
            "store.go": (
                "package judge\n\n"
                "type KVStore interface { Get(k string) string; Set(k, v string) }\n\n"
                "type MemStore struct { m map[string]string }\n"
                "func NewMemStore() *MemStore { return &MemStore{m: make(map[string]string)} }\n"
                "func (s *MemStore) Get(k string) string { return s.m[k] }\n"
                "func (s *MemStore) Set(k, v string) { s.m[k] = v }\n"
            ),
        },
        hidden_files={
            "hid_test.go": (
                "package judge\nimport \"testing\"\n"
                "func TestHid(t *testing.T) {\n"
                "    var _ KVStore = NewMemStore()\n"
                "}\n"
            ),
        },
        public_verify=["go test -run TestPub ."],
        hidden_verify=["go test -run TestHid ."],
        regression_verify=["go test -run TestReg ."],
    )

    # 20. judge-go-m-02 (M, tests)
    create_judging_task(
        name="judge-go-m-02",
        language="go",
        size="M",
        category="tests",
        objective="Implement wildcard pattern matching in router.go",
        setup_files={
            "go.mod": go_mod,
            "router.go": (
                "package judge\n\n"
                "func MatchRoute(pattern, path string) bool {\n"
                "    return pattern == path\n"
                "}\n"
            ),
            "reg_test.go": "package judge\nimport \"testing\"\nfunc TestReg(t *testing.T) { if !MatchRoute(\"/a\", \"/a\") { t.Fail() } }\n",
            "pub_test.go": "package judge\nimport \"testing\"\nfunc TestPub(t *testing.T) { if !MatchRoute(\"*\", \"/any\") { t.Fail() } }\n",
        },
        reference_files={
            "router.go": (
                "package judge\n\n"
                "func MatchRoute(pattern, path string) bool {\n"
                "    return pattern == path || pattern == \"*\"\n"
                "}\n"
            ),
        },
        hidden_files={
            "hid_test.go": "package judge\nimport \"testing\"\nfunc TestHid(t *testing.T) { if !MatchRoute(\"*\", \"/test\") { t.Fail() } }\n",
        },
        public_verify=["go test -run TestPub ."],
        hidden_verify=["go test -run TestHid ."],
        regression_verify=["go test -run TestReg ."],
    )

    # 21. judge-go-m-03 (M, boilerplate) [HOLDOUT]
    create_judging_task(
        name="judge-go-m-03",
        language="go",
        size="M",
        category="boilerplate",
        objective="Implement Validate method on Config in config.go",
        setup_files={
            "go.mod": go_mod,
            "config.go": (
                "package judge\n\n"
                "type Config struct { Port int; Host string }\n"
                "func (c *Config) Validate() error { return nil }\n"
            ),
            "reg_test.go": "package judge\nimport \"testing\"\nfunc TestReg(t *testing.T) { c := Config{Port: 8080}; if c.Port != 8080 { t.Fail() } }\n",
            "pub_test.go": (
                "package judge\nimport \"testing\"\nfunc TestPub(t *testing.T) {\n"
                "    c := Config{Port: -1}\n"
                "    if c.Validate() == nil { t.Fail() }\n"
                "}\n"
            ),
        },
        reference_files={
            "config.go": (
                "package judge\nimport \"errors\"\n\n"
                "type Config struct { Port int; Host string }\n\n"
                "func (c *Config) Validate() error {\n"
                "    if c.Port <= 0 || c.Port > 65535 { return errors.New(\"invalid port\") }\n"
                "    if c.Host == \"\" { return errors.New(\"empty host\") }\n"
                "    return nil\n"
                "}\n"
            ),
        },
        hidden_files={
            "hid_test.go": (
                "package judge\nimport \"testing\"\nfunc TestHid(t *testing.T) {\n"
                "    c := Config{Port: 0}\n"
                "    if c.Validate() == nil { t.Fail() }\n"
                "}\n"
            ),
        },
        public_verify=["go test -run TestPub ."],
        hidden_verify=["go test -run TestHid ."],
        regression_verify=["go test -run TestReg ."],
    )

    # 22. judge-go-l-01 (L, multi_file)
    create_judging_task(
        name="judge-go-l-01",
        language="go",
        size="L",
        category="multi_file",
        objective="Implement metrics pipeline across metrics.go, collector.go, and sink.go",
        setup_files={
            "go.mod": go_mod,
            "metrics.go": "package judge\n\ntype Metric struct { Name string; Value float64 }\n",
            "sink.go": "package judge\n\ntype Sink interface { Write(m Metric) }\n",
            "collector.go": (
                "package judge\n\n"
                "type Collector struct { Sinks []Sink }\n"
                "func (c *Collector) Emit(m Metric) {}\n"
            ),
            "reg_test.go": "package judge\nimport \"testing\"\nfunc TestReg(t *testing.T) { c := &Collector{}; c.Emit(Metric{}); if len(c.Sinks) != 0 { t.Fail() } }\n",
            "pub_test.go": (
                "package judge\nimport \"testing\"\n"
                "type mockSink struct { count int }\n"
                "func (m *mockSink) Write(_ Metric) { m.count++ }\n"
                "type sinkAdder interface { AddSink(s Sink) }\n"
                "func TestPub(t *testing.T) {\n"
                "    s := &mockSink{}\n"
                "    c := &Collector{}\n"
                "    if sa, ok := any(c).(sinkAdder); !ok {\n"
                "        t.Fail()\n"
                "    } else {\n"
                "        sa.AddSink(s)\n"
                "        c.Emit(Metric{Name: \"cpu\", Value: 10.0})\n"
                "        if s.count != 1 { t.Fail() }\n"
                "    }\n"
                "}\n"
            ),
        },
        reference_files={
            "collector.go": (
                "package judge\n\n"
                "type Collector struct { Sinks []Sink }\n"
                "func (c *Collector) AddSink(s Sink) { c.Sinks = append(c.Sinks, s) }\n"
                "func (c *Collector) Emit(m Metric) {\n"
                "    for _, s := range c.Sinks { s.Write(m) }\n"
                "}\n"
            ),
        },
        hidden_files={
            "hid_test.go": (
                "package judge\nimport \"testing\"\n"
                "func TestHid(t *testing.T) {\n"
                "    s := &mockSink{}\n"
                "    c := &Collector{}\n"
                "    c.AddSink(s)\n"
                "    c.Emit(Metric{Name: \"mem\", Value: 5.0})\n"
                "    if s.count != 1 { t.Fail() }\n"
                "}\n"
            ),
        },
        public_verify=["go test -run TestPub ."],
        hidden_verify=["go test -run TestHid ."],
        regression_verify=["go test -run TestReg ."],
    )

    # 23. judge-go-l-02 (L, multi_file)
    create_judging_task(
        name="judge-go-l-02",
        language="go",
        size="L",
        category="multi_file",
        objective="Implement worker queue processing across queue.go, worker.go, and pool.go",
        setup_files={
            "go.mod": go_mod,
            "queue.go": (
                "package judge\n\n"
                "type Queue struct { items []string }\n"
                "func (q *Queue) Push(s string) { q.items = append(q.items, s) }\n"
                "func (q *Queue) Pop() (string, bool) {\n"
                "    if len(q.items) == 0 { return \"\", false }\n"
                "    item := q.items[0]\n"
                "    q.items = q.items[1:]\n"
                "    return item, true\n"
                "}\n"
            ),
            "worker.go": "package judge\n\ntype Worker struct { ID int }\n",
            "pool.go": (
                "package judge\n\n"
                "type Pool struct { Q *Queue }\n"
                "func (p *Pool) ProcessNext() bool { return false }\n"
            ),
            "reg_test.go": "package judge\nimport \"testing\"\nfunc TestReg(t *testing.T) { q := &Queue{}; q.Push(\"x\"); if item, _ := q.Pop(); item != \"x\" { t.Fail() } }\n",
            "pub_test.go": "package judge\nimport \"testing\"\nfunc TestPub(t *testing.T) { p := &Pool{Q: &Queue{}}; p.Q.Push(\"job\"); if !p.ProcessNext() { t.Fail() } }\n",
        },
        reference_files={
            "pool.go": (
                "package judge\n\n"
                "type Pool struct { Q *Queue }\n"
                "func (p *Pool) ProcessNext() bool {\n"
                "    _, ok := p.Q.Pop()\n"
                "    return ok\n"
                "}\n"
            ),
        },
        hidden_files={
            "hid_test.go": "package judge\nimport \"testing\"\nfunc TestHid(t *testing.T) { p := &Pool{Q: &Queue{}}; p.Q.Push(\"job2\"); if !p.ProcessNext() { t.Fail() } }\n",
        },
        public_verify=["go test -run TestPub ."],
        hidden_verify=["go test -run TestHid ."],
        regression_verify=["go test -run TestReg ."],
    )

    # ==================== TYPESCRIPT (7 tasks) ====================
    # 24. judge-ts-s-01 (S, single_function)
    create_judging_task(
        name="judge-ts-s-01",
        language="typescript",
        size="S",
        category="single_function",
        objective="Implement slugify in slug.ts",
        setup_files={
            "slug.ts": "export function slugify(text: string): string {\n    return \"\";\n}\n",
            "test_reg.ts": "import { slugify } from \"./slug.ts\";\nif (slugify(\"\") !== \"\") process.exit(1);\n",
            "test_pub.ts": "import { slugify } from \"./slug.ts\";\nif (slugify(\"Hello World\") !== \"hello-world\") process.exit(1);\n",
        },
        reference_files={
            "slug.ts": (
                "export function slugify(text: string): string {\n"
                "    return text.toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');\n"
                "}\n"
            ),
        },
        hidden_files={
            "test_hid.ts": "import { slugify } from \"./slug.ts\";\nif (slugify(\"  Special & Chars! \") !== \"special-chars\") process.exit(1);\n",
        },
        public_verify=["node --experimental-strip-types test_pub.ts"],
        hidden_verify=["node --experimental-strip-types test_hid.ts"],
        regression_verify=["node --experimental-strip-types test_reg.ts"],
    )

    # 25. judge-ts-s-02 (S, local_bugfix)
    create_judging_task(
        name="judge-ts-s-02",
        language="typescript",
        size="S",
        category="local_bugfix",
        objective="Fix deepClone in clone.ts to preserve Arrays as Arrays instead of Objects",
        setup_files={
            "clone.ts": (
                "export function deepClone<T>(obj: T): T {\n"
                "    if (obj === null || typeof obj !== 'object') return obj;\n"
                "    const copy: any = {}; // bug: turns arrays into {}\n"
                "    for (const key of Object.keys(obj)) {\n"
                "        copy[key] = deepClone((obj as any)[key]);\n"
                "    }\n"
                "    return copy;\n"
                "}\n"
            ),
            "test_reg.ts": "import { deepClone } from \"./clone.ts\";\nconst a = { x: 1 };\nif (deepClone(a).x !== 1) process.exit(1);\n",
            "test_pub.ts": "import { deepClone } from \"./clone.ts\";\nconst res = deepClone([1, 2]);\nif (!Array.isArray(res)) process.exit(1);\n",
        },
        reference_files={
            "clone.ts": (
                "export function deepClone<T>(obj: T): T {\n"
                "    if (obj === null || typeof obj !== 'object') return obj;\n"
                "    if (Array.isArray(obj)) return obj.map(deepClone) as any;\n"
                "    const copy: any = {};\n"
                "    for (const key of Object.keys(obj)) {\n"
                "        copy[key] = deepClone((obj as any)[key]);\n"
                "    }\n"
                "    return copy;\n"
                "}\n"
            ),
        },
        hidden_files={
            "test_hid.ts": "import { deepClone } from \"./clone.ts\";\nconst res = deepClone({ items: [1, 2, 3] });\nif (!Array.isArray(res.items) || res.items.length !== 3) process.exit(1);\n",
        },
        public_verify=["node --experimental-strip-types test_pub.ts"],
        hidden_verify=["node --experimental-strip-types test_hid.ts"],
        regression_verify=["node --experimental-strip-types test_reg.ts"],
    )

    # 26. judge-ts-m-01 (M, local_refactor)
    create_judging_task(
        name="judge-ts-m-01",
        language="typescript",
        size="M",
        category="local_refactor",
        objective="Refactor loadData in api.ts to return a Promise instead of taking a callback",
        setup_files={
            "api.ts": (
                "export function loadData(id: string, cb: (res: string) => void): void {\n"
                "    cb(`data-${id}`);\n"
                "}\n"
            ),
            "test_reg.ts": "import { loadData } from \"./api.ts\";\nloadData(\"1\", (r) => { if (r !== \"data-1\") process.exit(1); });\n",
            "test_pub.ts": "import { loadData } from \"./api.ts\";\nconst p = (loadData as any)(\"2\");\nif (!(p instanceof Promise)) process.exit(1);\n",
        },
        reference_files={
            "api.ts": (
                "export function loadData(id: string, cb?: (res: string) => void): Promise<string> {\n"
                "    const val = `data-${id}`;\n"
                "    if (cb) cb(val);\n"
                "    return Promise.resolve(val);\n"
                "}\n"
            ),
        },
        hidden_files={
            "test_hid.ts": "import { loadData } from \"./api.ts\";\nloadData(\"3\").then(res => { if (res !== \"data-3\") process.exit(1); });\n",
        },
        public_verify=["node --experimental-strip-types test_pub.ts"],
        hidden_verify=["node --experimental-strip-types test_hid.ts"],
        regression_verify=["node --experimental-strip-types test_reg.ts"],
    )

    # 27. judge-ts-m-02 (M, tests)
    create_judging_task(
        name="judge-ts-m-02",
        language="typescript",
        size="M",
        category="tests",
        objective="Add test cases for isHttpError in test_errors.ts",
        setup_files={
            "errors.ts": "export function isHttpError(status: number): boolean {\n    return status >= 400 && status < 600;\n}\n",
            "test_errors.ts": "// TODO: tests\n",
            "test_reg.ts": "import { isHttpError } from \"./errors.ts\";\nif (!isHttpError(404)) process.exit(1);\n",
            "test_pub.ts": "import * as t from \"./test_errors.ts\";\nif (typeof (t as any).testServerErrors !== 'function') process.exit(1);\n(t as any).testServerErrors();\n",
        },
        reference_files={
            "test_errors.ts": (
                "import { isHttpError } from \"./errors.ts\";\n"
                "export function testServerErrors(): void {\n"
                "    if (!isHttpError(500) || !isHttpError(503)) process.exit(1);\n"
                "}\n"
                "export function testClientErrors(): void {\n"
                "    if (!isHttpError(400) || !isHttpError(401)) process.exit(1);\n"
                "}\n"
            ),
        },
        hidden_files={
            "test_hid.ts": "import * as t from \"./test_errors.ts\";\nif (typeof (t as any).testClientErrors !== 'function') process.exit(1);\n(t as any).testClientErrors();\n",
        },
        public_verify=["node --experimental-strip-types test_pub.ts"],
        hidden_verify=["node --experimental-strip-types test_hid.ts"],
        regression_verify=["node --experimental-strip-types test_reg.ts"],
    )

    # 28. judge-ts-m-03 (M, docs) [HOLDOUT]
    create_judging_task(
        name="judge-ts-m-03",
        language="typescript",
        size="M",
        category="docs",
        objective="Export VectorIdentifier and add Vector interface in math.ts",
        setup_files={
            "math.ts": "export function addVec(v1: any, v2: any): any {\n    return { x: v1.x + v2.x, y: v1.y + v2.y };\n}\n",
            "test_reg.ts": "import { addVec } from \"./math.ts\";\nconst r = addVec({x: 1, y: 2}, {x: 3, y: 4});\nif (r.x !== 4 || r.y !== 6) process.exit(1);\n",
            "test_pub.ts": "import * as m from \"./math.ts\";\nif (!(m as any).VectorIdentifier) process.exit(1);\n",
        },
        reference_files={
            "math.ts": (
                "export interface Vector { x: number; y: number; }\n"
                "export const VectorIdentifier = true;\n"
                "export function addVec(v1: Vector, v2: Vector): Vector {\n"
                "    return { x: v1.x + v2.x, y: v1.y + v2.y };\n"
                "}\n"
            ),
        },
        hidden_files={
            "test_hid.ts": "import * as m from \"./math.ts\";\nif ((m as any).VectorIdentifier !== true) process.exit(1);\n",
        },
        public_verify=["node --experimental-strip-types test_pub.ts"],
        hidden_verify=["node --experimental-strip-types test_hid.ts"],
        regression_verify=["node --experimental-strip-types test_reg.ts"],
    )

    # 29. judge-ts-l-01 (L, multi_file)
    create_judging_task(
        name="judge-ts-l-01",
        language="typescript",
        size="L",
        category="multi_file",
        objective="Implement typed store across store.ts, actions.ts, and reducer.ts",
        setup_files={
            "actions.ts": "export interface Action { type: string; }\nexport const ActionConst = true;\n",
            "reducer.ts": (
                "import type { Action } from './actions.ts';\n"
                "export function reducer(state: number, action: Action): number { return state; }\n"
            ),
            "store.ts": (
                "import type { Action } from './actions.ts';\n"
                "import { reducer } from './reducer.ts';\n"
                "export class Store {\n"
                "    state: number = 0;\n"
                "    dispatch(action: Action): void { this.state = reducer(this.state, action); }\n"
                "}\n"
            ),
            "test_reg.ts": "import { Store } from './store.ts';\nconst s = new Store();\nif (s.state !== 0) process.exit(1);\n",
            "test_pub.ts": "import { Store } from './store.ts';\nconst s = new Store();\ns.dispatch({ type: 'INCREMENT' });\nif (s.state !== 1) process.exit(1);\n",
        },
        reference_files={
            "reducer.ts": (
                "import type { Action } from './actions.ts';\n"
                "export function reducer(state: number, action: Action): number {\n"
                "    switch (action.type) {\n"
                "        case 'INCREMENT': return state + 1;\n"
                "        case 'DECREMENT': return state - 1;\n"
                "        default: return state;\n"
                "    }\n"
                "}\n"
            ),
        },
        hidden_files={
            "test_hid.ts": "import { Store } from './store.ts';\nconst s = new Store();\ns.dispatch({ type: 'DECREMENT' });\nif (s.state !== -1) process.exit(1);\n",
        },
        public_verify=["node --experimental-strip-types test_pub.ts"],
        hidden_verify=["node --experimental-strip-types test_hid.ts"],
        regression_verify=["node --experimental-strip-types test_reg.ts"],
    )

    # 30. judge-ts-l-02 (L, multi_file) [HOLDOUT]
    create_judging_task(
        name="judge-ts-l-02",
        language="typescript",
        size="L",
        category="multi_file",
        objective="Implement validation pipeline across schema.ts, rules.ts, and validator.ts",
        setup_files={
            "rules.ts": "export function isEmail(s: string): boolean { return s.includes('@'); }\n",
            "schema.ts": "export interface UserSchema { email: string; }\nexport const SchemaVersion = 1;\n",
            "validator.ts": (
                "import type { UserSchema } from './schema.ts';\n"
                "import { isEmail } from './rules.ts';\n"
                "export function validateUser(u: UserSchema): boolean { return false; }\n"
            ),
            "test_reg.ts": "import { isEmail } from './rules.ts';\nif (!isEmail('a@b.com')) process.exit(1);\n",
            "test_pub.ts": "import { validateUser } from './validator.ts';\nif (!validateUser({ email: 'test@domain.com' })) process.exit(1);\n",
        },
        reference_files={
            "validator.ts": (
                "import type { UserSchema } from './schema.ts';\n"
                "import { isEmail } from './rules.ts';\n"
                "export function validateUser(u: UserSchema): boolean {\n"
                "    return isEmail(u.email);\n"
                "}\n"
            ),
        },
        hidden_files={
            "test_hid.ts": "import { validateUser } from './validator.ts';\nif (validateUser({ email: 'invalid' })) process.exit(1);\nif (!validateUser({ email: 'valid@domain.com' })) process.exit(1);\n",
        },
        public_verify=["node --experimental-strip-types test_pub.ts"],
        hidden_verify=["node --experimental-strip-types test_hid.ts"],
        regression_verify=["node --experimental-strip-types test_reg.ts"],
    )

    # Write holdout.json
    holdout = {
        "holdout": [
            "judge-py-s-03",
            "judge-py-l-02",
            "judge-rs-s-03",
            "judge-go-m-03",
            "judge-ts-m-03",
            "judge-ts-l-02",
        ]
    }
    (JUDGING_DIR / "holdout.json").write_text(json.dumps(holdout, indent=2), encoding="utf-8")
    print(f"Generated 30 judging tasks with 6 holdouts in {JUDGING_DIR}")


if __name__ == "__main__":
    build_all_judging_tasks()

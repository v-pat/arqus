"""Generates the 60-contract nano set for arqus flow."""

from __future__ import annotations

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from evals.generate_task_sets import create_nano_task, NANO_DIR


def build_all_nano_tasks():
    print("Generating 60 nano tasks...")

    # 1. BOILERPLATE (8 tasks)
    boilerplate_defs = [
        ("nano-bp-01", "Implement to_dict on Point in geom.py", "geom.py",
         "class Point:\n    def __init__(self, x, y): self.x = x; self.y = y\n",
         "class Point:\n    def __init__(self, x, y): self.x = x; self.y = y\n    def to_dict(self): return {'x': self.x, 'y': self.y}\n",
         "import geom; p = geom.Point(1, 2); assert hasattr(p, 'to_dict') and p.to_dict() == {'x': 1, 'y': 2}"),
        ("nano-bp-02", "Implement repr on Color in color.py", "color.py",
         "class Color:\n    def __init__(self, r, g, b): self.r = r; self.g = g; self.b = b\n",
         "class Color:\n    def __init__(self, r, g, b): self.r = r; self.g = g; self.b = b\n    def __repr__(self): return f'Color({self.r}, {self.g}, {self.b})'\n",
         "import color; c = color.Color(255, 0, 0); assert repr(c) == 'Color(255, 0, 0)'"),
        ("nano-bp-03", "Implement equality on Vector2D in vec.py", "vec.py",
         "class Vector2D:\n    def __init__(self, x, y): self.x = x; self.y = y\n",
         "class Vector2D:\n    def __init__(self, x, y): self.x = x; self.y = y\n    def __eq__(self, o): return isinstance(o, Vector2D) and (self.x, self.y) == (o.x, o.y)\n",
         "import vec; assert vec.Vector2D(1, 2) == vec.Vector2D(1, 2) and not (vec.Vector2D(1, 2) == vec.Vector2D(2, 1))"),
        ("nano-bp-04", "Implement hash on Symbol in sym.py", "sym.py",
         "class Symbol:\n    def __init__(self, name): self.name = name\n    def __eq__(self, o): return isinstance(o, Symbol) and self.name == o.name\n",
         "class Symbol:\n    def __init__(self, name): self.name = name\n    def __eq__(self, o): return isinstance(o, Symbol) and self.name == o.name\n    def __hash__(self): return hash(self.name)\n",
         "import sym; s = {sym.Symbol('a'), sym.Symbol('a')}; assert len(s) == 1"),
        ("nano-bp-05", "Implement copy on Config in config.py", "config.py",
         "class Config:\n    def __init__(self, host, port): self.host = host; self.port = port\n",
         "class Config:\n    def __init__(self, host, port): self.host = host; self.port = port\n    def copy(self): return Config(self.host, self.port)\n",
         "import config; c = config.Config('loc', 80); c2 = c.copy(); assert c2 is not c and c2.port == 80"),
        ("nano-bp-06", "Implement len on Buffer in buf.py", "buf.py",
         "class Buffer:\n    def __init__(self, items): self.items = list(items)\n",
         "class Buffer:\n    def __init__(self, items): self.items = list(items)\n    def __len__(self): return len(self.items)\n",
         "import buf; b = buf.Buffer([1, 2, 3]); assert len(b) == 3"),
        ("nano-bp-07", "Implement contains on Range in rng.py", "rng.py",
         "class Range:\n    def __init__(self, low, high): self.low = low; self.high = high\n",
         "class Range:\n    def __init__(self, low, high): self.low = low; self.high = high\n    def __contains__(self, v): return self.low <= v <= self.high\n",
         "import rng; r = rng.Range(10, 20); assert 15 in r and 5 not in r"),
        ("nano-bp-08", "Implement iter on Queue in queue.py", "queue.py",
         "class Queue:\n    def __init__(self): self.q = [1, 2, 3]\n",
         "class Queue:\n    def __init__(self): self.q = [1, 2, 3]\n    def __iter__(self): return iter(self.q)\n",
         "import queue; q = queue.Queue(); assert list(q) == [1, 2, 3]"),
    ]

    for idx, (name, obj, fname, setup_code, ref_code, test_code) in enumerate(boilerplate_defs, 1):
        create_nano_task(
            name=name,
            category="boilerplate",
            objective=obj,
            context_files=[fname],
            allowed_writes=[fname],
            verify_command=f"python3 test_hid.py",
            setup_files={fname: setup_code, "test_hid.py": test_code + "\n"},
            reference_files={fname: ref_code},
            hidden_verify=["python3 test_hid.py"],
        )

    # 2. SINGLE_FUNCTION (8 tasks)
    single_func_defs = [
        ("nano-sf-01", "Implement square(x)", "math_fn.py",
         "def square(x): pass\n", "def square(x): return x * x\n",
         "import math_fn; assert math_fn.square(5) == 25"),
        ("nano-sf-02", "Implement is_even(n)", "even.py",
         "def is_even(n): pass\n", "def is_even(n): return n % 2 == 0\n",
         "import even; assert even.is_even(4) is True and even.is_even(7) is False"),
        ("nano-sf-03", "Implement capitalize_words(s)", "text_fn.py",
         "def capitalize_words(s): pass\n", "def capitalize_words(s): return ' '.join(w.capitalize() for w in s.split())\n",
         "import text_fn; assert text_fn.capitalize_words('hello world') == 'Hello World'"),
        ("nano-sf-04", "Implement abs_diff(a, b)", "diff.py",
         "def abs_diff(a, b): pass\n", "def abs_diff(a, b): return abs(a - b)\n",
         "import diff; assert diff.abs_diff(10, 15) == 5 and diff.abs_diff(20, 5) == 15"),
        ("nano-sf-05", "Implement sum_positive(lst)", "list_fn.py",
         "def sum_positive(lst): pass\n", "def sum_positive(lst): return sum(x for x in lst if x > 0)\n",
         "import list_fn; assert list_fn.sum_positive([-1, 2, -3, 4]) == 6"),
        ("nano-sf-06", "Implement repeat_str(s, n)", "str_fn.py",
         "def repeat_str(s, n): pass\n", "def repeat_str(s, n): return s * n\n",
         "import str_fn; assert str_fn.repeat_str('ab', 3) == 'ababab'"),
        ("nano-sf-07", "Implement gcd(a, b)", "gcd.py",
         "def gcd(a, b): pass\n", "import math\ndef gcd(a, b): return math.gcd(a, b)\n",
         "import gcd; assert gcd.gcd(12, 18) == 6"),
        ("nano-sf-08", "Implement is_vowel(c)", "char_fn.py",
         "def is_vowel(c): pass\n", "def is_vowel(c): return c.lower() in 'aeiou'\n",
         "import char_fn; assert char_fn.is_vowel('A') is True and char_fn.is_vowel('x') is False"),
    ]

    for name, obj, fname, setup_code, ref_code, test_code in single_func_defs:
        create_nano_task(
            name=name,
            category="single_function",
            objective=obj,
            context_files=[fname],
            allowed_writes=[fname],
            verify_command="python3 test_hid.py",
            setup_files={fname: setup_code, "test_hid.py": test_code + "\n"},
            reference_files={fname: ref_code},
            hidden_verify=["python3 test_hid.py"],
        )

    # 3. TESTS (8 tasks)
    test_defs = [
        ("nano-tests-01", "Add test_add in test_calc.py", "test_calc.py",
         "# TODO\n", "def test_add(): assert 1 + 1 == 2\n",
         "import test_calc; assert hasattr(test_calc, 'test_add'); test_calc.test_add()"),
        ("nano-tests-02", "Add test_mul in test_mul.py", "test_mul.py",
         "# TODO\n", "def test_mul(): assert 3 * 4 == 12\n",
         "import test_mul; assert hasattr(test_mul, 'test_mul'); test_mul.test_mul()"),
        ("nano-tests-03", "Add test_sub in test_sub.py", "test_sub.py",
         "# TODO\n", "def test_sub(): assert 10 - 4 == 6\n",
         "import test_sub; assert hasattr(test_sub, 'test_sub'); test_sub.test_sub()"),
        ("nano-tests-04", "Add test_div in test_div.py", "test_div.py",
         "# TODO\n", "def test_div(): assert 12 / 3 == 4\n",
         "import test_div; assert hasattr(test_div, 'test_div'); test_div.test_div()"),
        ("nano-tests-05", "Add test_upper in test_str.py", "test_str.py",
         "# TODO\n", "def test_upper(): assert 'abc'.upper() == 'ABC'\n",
         "import test_str; assert hasattr(test_str, 'test_upper'); test_str.test_upper()"),
        ("nano-tests-06", "Add test_lower in test_str.py", "test_str.py",
         "# TODO\n", "def test_lower(): assert 'ABC'.lower() == 'abc'\n",
         "import test_str; assert hasattr(test_str, 'test_lower'); test_str.test_lower()"),
        ("nano-tests-07", "Add test_split in test_split.py", "test_split.py",
         "# TODO\n", "def test_split(): assert 'a,b'.split(',') == ['a', 'b']\n",
         "import test_split; assert hasattr(test_split, 'test_split'); test_split.test_split()"),
        ("nano-tests-08", "Add test_join in test_join.py", "test_join.py",
         "# TODO\n", "def test_join(): assert '-'.join(['x', 'y']) == 'x-y'\n",
         "import test_join; assert hasattr(test_join, 'test_join'); test_join.test_join()"),
    ]

    for name, obj, fname, setup_code, ref_code, test_code in test_defs:
        create_nano_task(
            name=name,
            category="tests",
            objective=obj,
            context_files=[fname],
            allowed_writes=[fname],
            verify_command="python3 test_hid.py",
            setup_files={fname: setup_code, "test_hid.py": test_code + "\n"},
            reference_files={fname: ref_code},
            hidden_verify=["python3 test_hid.py"],
        )

    # 4. DOCS (8 tasks)
    docs_defs = [
        ("nano-docs-01", "Add docstring to add in calc.py", "calc.py",
         "def add(a, b): return a + b\n",
         "def add(a, b):\n    \"\"\"Add two numbers.\"\"\"\n    return a + b\n",
         "import calc; assert calc.add.__doc__ is not None and 'Add' in calc.add.__doc__"),
        ("nano-docs-02", "Add docstring to mul in math_ops.py", "math_ops.py",
         "def mul(a, b): return a * b\n",
         "def mul(a, b):\n    \"\"\"Multiply two numbers.\"\"\"\n    return a * b\n",
         "import math_ops; assert math_ops.mul.__doc__ is not None and 'Multiply' in math_ops.mul.__doc__"),
        ("nano-docs-03", "Add module docstring to mod.py", "mod.py",
         "x = 1\n",
         "\"\"\"Module utilities.\"\"\"\nx = 1\n",
         "import mod; assert mod.__doc__ is not None and 'Module' in mod.__doc__"),
        ("nano-docs-04", "Add docstring to User class in user.py", "user.py",
         "class User: pass\n",
         "class User:\n    \"\"\"Represents a user account.\"\"\"\n    pass\n",
         "import user; assert user.User.__doc__ is not None and 'user' in user.User.__doc__.lower()"),
        ("nano-docs-05", "Add docstring to greet in hello.py", "hello.py",
         "def greet(name): return f'Hello, {name}'\n",
         "def greet(name):\n    \"\"\"Return greeting for name.\"\"\"\n    return f'Hello, {name}'\n",
         "import hello; assert hello.greet.__doc__ is not None and 'greeting' in hello.greet.__doc__.lower()"),
        ("nano-docs-06", "Add docstring to factorial in fact.py", "fact.py",
         "def fact(n): return 1 if n <= 1 else n * fact(n-1)\n",
         "def fact(n):\n    \"\"\"Compute factorial of n.\"\"\"\n    return 1 if n <= 1 else n * fact(n-1)\n",
         "import fact; assert fact.fact.__doc__ is not None and 'factorial' in fact.fact.__doc__.lower()"),
        ("nano-docs-07", "Add docstring to reverse_str in rev.py", "rev.py",
         "def reverse_str(s): return s[::-1]\n",
         "def reverse_str(s):\n    \"\"\"Reverse string.\"\"\"\n    return s[::-1]\n",
         "import rev; assert rev.reverse_str.__doc__ is not None and 'Reverse' in rev.reverse_str.__doc__"),
        ("nano-docs-08", "Add docstring to is_zero in num.py", "num.py",
         "def is_zero(n): return n == 0\n",
         "def is_zero(n):\n    \"\"\"Check if number is zero.\"\"\"\n    return n == 0\n",
         "import num; assert num.is_zero.__doc__ is not None and 'zero' in num.is_zero.__doc__.lower()"),
    ]

    for name, obj, fname, setup_code, ref_code, test_code in docs_defs:
        create_nano_task(
            name=name,
            category="docs",
            objective=obj,
            context_files=[fname],
            allowed_writes=[fname],
            verify_command="python3 test_hid.py",
            setup_files={fname: setup_code, "test_hid.py": test_code + "\n"},
            reference_files={fname: ref_code},
            hidden_verify=["python3 test_hid.py"],
        )

    # 5. LOCAL_REFACTOR (8 tasks)
    refactor_defs = [
        ("nano-ref-01", "Refactor square_all using list comprehension in trans.py", "trans.py",
         "def square_all(nums):\n    res = []\n    for n in nums: res.append(n * n)\n    return res\n",
         "def square_all(nums):\n    return [n * n for n in nums]\n",
         "import trans, inspect; assert trans.square_all([1, 2, 3]) == [1, 4, 9]; src = inspect.getsource(trans.square_all); assert 'for' in src and 'append' not in src"),
        ("nano-ref-02", "Refactor filter_pos using list comprehension in filt.py", "filt.py",
         "def filter_pos(nums):\n    res = []\n    for n in nums:\n        if n > 0: res.append(n)\n    return res\n",
         "def filter_pos(nums):\n    return [n for n in nums if n > 0]\n",
         "import filt, inspect; assert filt.filter_pos([-1, 2, -3, 4]) == [2, 4]; src = inspect.getsource(filt.filter_pos); assert 'append' not in src"),
        ("nano-ref-03", "Refactor sign using ternary in sign.py", "sign.py",
         "def sign(x):\n    if x >= 0:\n        return 1\n    else:\n        return -1\n",
         "def sign(x):\n    return 1 if x >= 0 else -1\n",
         "import sign, inspect; assert sign.sign(5) == 1 and sign.sign(-2) == -1; src = inspect.getsource(sign.sign); assert len(src.strip().splitlines()) <= 2"),
        ("nano-ref-04", "Refactor min_two using min built-in in min_util.py", "min_util.py",
         "def min_two(a, b):\n    if a < b:\n        return a\n    return b\n",
         "def min_two(a, b):\n    return min(a, b)\n",
         "import min_util, inspect; assert min_util.min_two(3, 7) == 3; src = inspect.getsource(min_util.min_two); assert 'min(' in src"),
        ("nano-ref-05", "Refactor max_two using max built-in in max_util.py", "max_util.py",
         "def max_two(a, b):\n    if a > b:\n        return a\n    return b\n",
         "def max_two(a, b):\n    return max(a, b)\n",
         "import max_util, inspect; assert max_util.max_two(3, 7) == 7; src = inspect.getsource(max_util.max_two); assert 'max(' in src"),
        ("nano-ref-06", "Refactor join_words using join in joiner.py", "joiner.py",
         "def join_words(words):\n    res = ''\n    for w in words: res += w + ' '\n    return res.strip()\n",
         "def join_words(words):\n    return ' '.join(words)\n",
         "import joiner, inspect; assert joiner.join_words(['a', 'b']) == 'a b'; src = inspect.getsource(joiner.join_words); assert '.join(' in src"),
        ("nano-ref-07", "Refactor count_items using len in counter.py", "counter.py",
         "def count_items(items):\n    c = 0\n    for _ in items: c += 1\n    return c\n",
         "def count_items(items):\n    return len(items)\n",
         "import counter, inspect; assert counter.count_items([1, 2, 3]) == 3; src = inspect.getsource(counter.count_items); assert 'len(' in src"),
        ("nano-ref-08", "Refactor check_all using all() in checker.py", "checker.py",
         "def check_all(flags):\n    for f in flags:\n        if not f: return False\n    return True\n",
         "def check_all(flags):\n    return all(flags)\n",
         "import checker, inspect; assert checker.check_all([True, True]) is True and checker.check_all([True, False]) is False; src = inspect.getsource(checker.check_all); assert 'return all(' in src"),
    ]

    for name, obj, fname, setup_code, ref_code, test_code in refactor_defs:
        create_nano_task(
            name=name,
            category="local_refactor",
            objective=obj,
            context_files=[fname],
            allowed_writes=[fname],
            verify_command="python3 test_hid.py",
            setup_files={fname: setup_code, "test_hid.py": test_code + "\n"},
            reference_files={fname: ref_code},
            hidden_verify=["python3 test_hid.py"],
        )

    # 6. LOCAL_BUGFIX (8 tasks)
    bugfix_defs = [
        ("nano-bf-01", "Fix zero division in safe_div in div.py", "div.py",
         "def safe_div(a, b):\n    return a / b\n",
         "def safe_div(a, b):\n    if b == 0: return 0\n    return a / b\n",
         "import div; assert div.safe_div(10, 2) == 5 and div.safe_div(10, 0) == 0"),
        ("nano-bf-02", "Fix index error on empty list in first_item in lst.py", "lst.py",
         "def first_item(items):\n    return items[0]\n",
         "def first_item(items):\n    return items[0] if items else None\n",
         "import lst; assert lst.first_item([1, 2]) == 1 and lst.first_item([]) is None"),
        ("nano-bf-03", "Fix negative index handling in clamp_index in idx.py", "idx.py",
         "def clamp_index(i, length):\n    if i >= length: return length - 1\n    return i\n",
         "def clamp_index(i, length):\n    if length <= 0: return 0\n    return max(0, min(i, length - 1))\n",
         "import idx; assert idx.clamp_index(-5, 10) == 0 and idx.clamp_index(15, 10) == 9"),
        ("nano-bf-04", "Fix key error in get_name in user_dict.py", "user_dict.py",
         "def get_name(d):\n    return d['name']\n",
         "def get_name(d):\n    return d.get('name', 'Anonymous')\n",
         "import user_dict; assert user_dict.get_name({'name': 'Sam'}) == 'Sam' and user_dict.get_name({}) == 'Anonymous'"),
        ("nano-bf-05", "Fix NoneTypeError in str_len in string_ops.py", "string_ops.py",
         "def str_len(s):\n    return len(s)\n",
         "def str_len(s):\n    return len(s) if s is not None else 0\n",
         "import string_ops; assert string_ops.str_len('hi') == 2 and string_ops.str_len(None) == 0"),
        ("nano-bf-06", "Fix off-by-one in in_bounds in bounds.py", "bounds.py",
         "def in_bounds(x, low, high):\n    return low < x < high\n",
         "def in_bounds(x, low, high):\n    return low <= x <= high\n",
         "import bounds; assert bounds.in_bounds(5, 5, 10) is True and bounds.in_bounds(10, 5, 10) is True"),
        ("nano-bf-07", "Fix mutable default argument in append_item in mut.py", "mut.py",
         "def append_item(val, items=[]):\n    items.append(val)\n    return items\n",
         "def append_item(val, items=None):\n    if items is None: items = []\n    items.append(val)\n    return items\n",
         "import mut; a = mut.append_item(1); b = mut.append_item(2); assert a == [1] and b == [2]"),
        ("nano-bf-08", "Fix float precision in is_close in math_cmp.py", "math_cmp.py",
         "def is_close(a, b):\n    return a == b\n",
         "def is_close(a, b):\n    return abs(a - b) < 1e-5\n",
         "import math_cmp; assert math_cmp.is_close(0.1 + 0.2, 0.3) is True"),
    ]

    for name, obj, fname, setup_code, ref_code, test_code in bugfix_defs:
        create_nano_task(
            name=name,
            category="local_bugfix",
            objective=obj,
            context_files=[fname],
            allowed_writes=[fname],
            verify_command="python3 test_hid.py",
            setup_files={fname: setup_code, "test_hid.py": test_code + "\n"},
            reference_files={fname: ref_code},
            hidden_verify=["python3 test_hid.py"],
        )

    # 7. MULTI_FILE (8 tasks)
    for i in range(1, 9):
        name = f"nano-mf-{i:02d}"
        obj = f"Update service in service_{i}.py and client in client_{i}.py"
        sname = f"service_{i}.py"
        cname = f"client_{i}.py"
        s_code_before = "class Service:\n    def call(self): return 'old'\n"
        s_code_after = "class Service:\n    def call(self): return 'updated'\n"
        c_code_before = f"from {sname[:-3]} import Service\nclass Client:\n    def run(self): return 'unconfigured'\n"
        c_code_after = f"from {sname[:-3]} import Service\nclass Client:\n    def run(self):\n        return Service().call()\n"
        test_code = f"import {cname[:-3]}; c = {cname[:-3]}.Client(); assert c.run() == 'updated'"

        create_nano_task(
            name=name,
            category="multi_file",
            objective=obj,
            context_files=[sname, cname],
            allowed_writes=[sname, cname],
            verify_command="python3 test_hid.py",
            setup_files={sname: s_code_before, cname: c_code_before, "test_hid.py": test_code + "\n"},
            reference_files={sname: s_code_after, cname: c_code_after},
            hidden_verify=["python3 test_hid.py"],
        )

    # 8. OTHER (4 tasks)
    other_defs = [
        ("nano-oth-01", "Configure logging level in env.py", "env.py",
         "LOG_LEVEL = 'INFO'\n", "LOG_LEVEL = 'DEBUG'\n",
         "import env; assert env.LOG_LEVEL == 'DEBUG'"),
        ("nano-oth-02", "Set app version in ver.py", "ver.py",
         "VERSION = '0.1.0'\n", "VERSION = '1.0.0'\n",
         "import ver; assert ver.VERSION == '1.0.0'"),
        ("nano-oth-03", "Update max retries in retry.py", "retry.py",
         "MAX_RETRIES = 3\n", "MAX_RETRIES = 5\n",
         "import retry; assert retry.MAX_RETRIES == 5"),
        ("nano-oth-04", "Toggle feature flag in flags.py", "flags.py",
         "ENABLE_METRICS = False\n", "ENABLE_METRICS = True\n",
         "import flags; assert flags.ENABLE_METRICS is True"),
    ]

    for name, obj, fname, setup_code, ref_code, test_code in other_defs:
        create_nano_task(
            name=name,
            category="other",
            objective=obj,
            context_files=[fname],
            allowed_writes=[fname],
            verify_command="python3 test_hid.py",
            setup_files={fname: setup_code, "test_hid.py": test_code + "\n"},
            reference_files={fname: ref_code},
            hidden_verify=["python3 test_hid.py"],
        )

    print(f"Generated 60 nano tasks in {NANO_DIR}")


if __name__ == "__main__":
    build_all_nano_tasks()

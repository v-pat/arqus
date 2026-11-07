import subprocess
import sys
import unittest


class CliTest(unittest.TestCase):
    def run_cli(self, *args):
        return subprocess.run(
            ["python3", "test_target.py", *args],
            capture_output=True,
            text=True,
        )

    def test_plain(self):
        result = self.run_cli("hello", "world")
        self.assertEqual(result.stdout.strip(), "hello world")

    def test_upper(self):
        result = self.run_cli("hello", "world", "--upper")
        self.assertEqual(result.stdout.strip(), "HELLO WORLD")


if __name__ == "__main__":
    unittest.main()

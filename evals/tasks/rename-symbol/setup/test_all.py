import unittest

from a import compute_total
from b import report
from c import double_total


class RenameTest(unittest.TestCase):
    def test_total(self):
        self.assertEqual(compute_total([1, 2, 3]), 6)

    def test_report(self):
        self.assertEqual(report([1, 2, 3]), "total=6")

    def test_double(self):
        self.assertEqual(double_total([1, 2, 3]), 12)


if __name__ == "__main__":
    unittest.main()

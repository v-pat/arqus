import unittest

from calc import add_range


class AddRangeTest(unittest.TestCase):
    def test_single_value(self):
        self.assertEqual(add_range(1, 1), 1)

    def test_small_range(self):
        self.assertEqual(add_range(1, 5), 15)

    def test_zero_start(self):
        self.assertEqual(add_range(0, 3), 6)


if __name__ == "__main__":
    unittest.main()

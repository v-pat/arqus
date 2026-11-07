import unittest

from util import double, slugify


class SlugifyTest(unittest.TestCase):
    def test_punctuation(self):
        self.assertEqual(slugify("Hello, World!"), "hello-world")

    def test_spaces(self):
        self.assertEqual(slugify("  A  B  "), "a-b")

    def test_existing_hyphen(self):
        self.assertEqual(slugify("Already-slugged"), "already-slugged")

    def test_underscore(self):
        self.assertEqual(slugify("x_y"), "x-y")


class DoubleTest(unittest.TestCase):
    def test_double(self):
        self.assertEqual(double(4), 8)


if __name__ == "__main__":
    unittest.main()

import sys


def render(text):
    return text


def main(argv):
    print(render(" ".join(argv[1:])))


if __name__ == "__main__":
    main(sys.argv)

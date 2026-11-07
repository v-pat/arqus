"""Small wrapper so the test can run cli.py without a package."""
import runpy

runpy.run_path("cli.py", run_name="__main__")

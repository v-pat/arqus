import rate_limit
class FakeClock:
    def now(self): return 500.0
b = rate_limit.TokenBucket(5, clock=FakeClock())
assert b.last == 500.0

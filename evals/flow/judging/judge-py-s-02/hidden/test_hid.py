import billing
try:
    billing.calc_discount(100, None)
    assert False, 'Should have raised ValueError'
except ValueError:
    pass

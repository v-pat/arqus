def add_range(start, stop):
    """Return the sum of the integers from start up to and including stop."""
    total = 0
    for value in range(start, stop):
        total += value
    return total

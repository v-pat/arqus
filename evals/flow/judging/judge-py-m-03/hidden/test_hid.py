import matrix_ops
ann = matrix_ops.dot_product.__annotations__
assert 'v1' in ann and 'return' in ann

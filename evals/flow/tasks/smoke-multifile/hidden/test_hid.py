import auth
assert auth.authenticate("alice", "secret123") is True
assert auth.authenticate("alice", "wrong") is False
assert auth.authenticate("bob", "secret123") is False


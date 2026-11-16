import models
u2 = models.User.from_dict({'username': 'carol', 'email': 'c@b.com'})
assert u2.username == 'carol' and u2.email == 'c@b.com'

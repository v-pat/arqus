from cache import TieredCache
c = TieredCache()
c.set('allowed_key', 'val')
assert c.get('allowed_key') == 'val'

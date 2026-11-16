from events import Event
e = Event('audit', {'user': 'root'})
assert e.payload == {'user': 'root'}

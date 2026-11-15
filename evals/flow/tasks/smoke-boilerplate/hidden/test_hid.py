import config
c = config.Config(host="127.0.0.1", port=8080)
assert c.host == "127.0.0.1" and c.port == 8080

